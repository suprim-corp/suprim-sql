//! Self-update install pipeline (macOS only for now — Linux/Windows follow
//! similar but distinct flows and are not yet implemented).
//!
//! Steps, each reported via [`UpdateProgress`]:
//!
//! 1. **Download** DMG to `~/Library/Caches/SuprimSQL/updates/<version>.dmg`.
//! 2. **Verify** the SHA-256 matches the feed's `sha256` field.
//! 3. **Mount** the DMG via `hdiutil attach -nobrowse -quiet`.
//! 4. **Copy** the nested `SuprimSQL.app` into `/Applications/SuprimSQL.app`.
//!    We delete the existing bundle first so `cp -R` doesn't produce a nested
//!    `SuprimSQL.app/SuprimSQL.app` directory.
//! 5. **Unmount** with `hdiutil detach`.
//! 6. **Relaunch**: `open -n /Applications/SuprimSQL.app` spawns the new
//!    instance, then we exit so the user sees it replace the current window.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use super::state::{set, SharedUpdateState};
use super::{LatestRelease, UpdateProgress, UpdateState};

const APP_INSTALL_PATH: &str = "/Applications/SuprimSQL.app";
const BUNDLED_APP_NAME: &str = "SuprimSQL.app";

/// Kick off the install pipeline. Consumes the release; the caller should
/// have stored it in `UpdateState::Available` first so the banner can keep
/// rendering its metadata while download is in progress.
pub async fn install_update(state: SharedUpdateState, release: LatestRelease) {
    let result = install_inner(&state, &release).await;

    match result {
        Ok(()) => {
            set(&state, UpdateState::Relaunching);
            // Give the user a beat to see "Relaunching…" before we disappear.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if let Err(e) = relaunch() {
                set(&state, UpdateState::Failed(format!("Relaunch failed: {e}")));
                return;
            }
            // Once `open -n` has spawned the new process, exit so macOS shifts
            // focus to the fresh window.
            std::process::exit(0);
        }
        Err(e) => {
            set(&state, UpdateState::Failed(e));
        }
    }
}

async fn install_inner(state: &SharedUpdateState, release: &LatestRelease) -> Result<(), String> {
    let dmg_path = cache_path(&release.version)?;
    download_dmg(state, release, &dmg_path).await?;

    set(
        state,
        UpdateState::Installing {
            release: release.clone(),
            progress: UpdateProgress::Verifying,
        },
    );
    verify_sha256(&dmg_path, &release.sha256)?;

    set(
        state,
        UpdateState::Installing {
            release: release.clone(),
            progress: UpdateProgress::Mounting,
        },
    );
    let mount_point = mount_dmg(&dmg_path)?;

    // Make sure we always unmount, even if the copy step fails.
    let copy_result = {
        set(
            state,
            UpdateState::Installing {
                release: release.clone(),
                progress: UpdateProgress::Copying,
            },
        );
        copy_app(&mount_point)
    };

    set(
        state,
        UpdateState::Installing {
            release: release.clone(),
            progress: UpdateProgress::Unmounting,
        },
    );
    let _ = unmount_dmg(&mount_point); // best-effort; log but don't fail the update

    copy_result
}

fn cache_path(version: &str) -> Result<PathBuf, String> {
    let root = dirs_next::cache_dir().ok_or_else(|| "cannot resolve cache dir".to_owned())?;
    cache_path_in(&root, version)
}

/// Testable variant: build the cache path under `root` instead of the
/// user's cache dir, so unit tests can point at a `tempfile::TempDir`.
fn cache_path_in(root: &Path, version: &str) -> Result<PathBuf, String> {
    let cache_dir = root.join("SuprimSQL").join("updates");
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir {cache_dir:?}: {e}"))?;
    Ok(cache_dir.join(format!("SuprimSQL-{version}.dmg")))
}

async fn download_dmg(
    state: &SharedUpdateState,
    release: &LatestRelease,
    dest: &Path,
) -> Result<(), String> {
    set(
        state,
        UpdateState::Installing {
            release: release.clone(),
            progress: UpdateProgress::Downloading {
                bytes_done: 0,
                bytes_total: release.size_bytes,
            },
        },
    );

    stream_to_file(&release.download_url, dest, release.size_bytes, |bytes_done, total| {
        set(
            state,
            UpdateState::Installing {
                release: release.clone(),
                progress: UpdateProgress::Downloading { bytes_done, bytes_total: total },
            },
        );
    })
    .await
}

/// Low-level streaming downloader. Writes `url` to `dest` a chunk at a
/// time, invoking `on_progress(bytes_done, bytes_total)` after each chunk.
/// Kept free of domain types so it's easy to unit-test against a local
/// wiremock server.
async fn stream_to_file<F>(
    url: &str,
    dest: &Path,
    fallback_total: u64,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64),
{
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download HTTP status: {e}"))?;

    let total = response.content_length().unwrap_or(fallback_total);
    let mut file = std::fs::File::create(dest).map_err(|e| format!("create {dest:?}: {e}"))?;
    let mut bytes_done: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download stream: {e}"))?;
        file.write_all(&chunk).map_err(|e| format!("write: {e}"))?;
        bytes_done += chunk.len() as u64;
        on_progress(bytes_done, total);
    }

    file.sync_all().map_err(|e| format!("fsync: {e}"))?;
    Ok(())
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let digest = Sha256::digest(&bytes);
    let actual_hex = hex_encode(&digest);
    if actual_hex.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        // Remove the corrupt file so a retry doesn't trust the cached copy.
        let _ = std::fs::remove_file(path);
        Err(format!(
            "checksum mismatch (expected {expected_hex}, got {actual_hex})"
        ))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Mount the DMG and return the resolved mount point (e.g. `/Volumes/SuprimSQL`).
/// Parses the XML plist output of `hdiutil attach -plist` to pick the
/// mount path without relying on predictable volume names.
fn mount_dmg(dmg: &Path) -> Result<PathBuf, String> {
    let output = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-plist"])
        .arg(dmg)
        .output()
        .map_err(|e| format!("spawn hdiutil: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let plist = String::from_utf8_lossy(&output.stdout);
    parse_mount_point(&plist)
        .map(PathBuf::from)
        .ok_or_else(|| "mount-point not found in hdiutil plist output".to_owned())
}

/// Pull the first `<key>mount-point</key><string>…</string>` pair out of the
/// XML plist hdiutil emits. Split off into a pure function so it's testable
/// without invoking `hdiutil`.
fn parse_mount_point(plist: &str) -> Option<String> {
    plist
        .split("<key>mount-point</key>")
        .nth(1)
        .and_then(|tail| tail.split("<string>").nth(1))
        .and_then(|tail| tail.split("</string>").next())
        .map(|s| s.trim().to_owned())
}

fn copy_app(mount_point: &Path) -> Result<(), String> {
    copy_app_to(mount_point, Path::new(APP_INSTALL_PATH))
}

/// Testable core of [`copy_app`]: copies `mount_point/BUNDLED_APP_NAME` into
/// `dest`, replacing it atomically (as far as `cp -R` allows). Extracted so
/// unit tests can point at a `tempfile::TempDir` instead of
/// `/Applications/`.
fn copy_app_to(mount_point: &Path, dest: &Path) -> Result<(), String> {
    let source = mount_point.join(BUNDLED_APP_NAME);
    if !source.exists() {
        return Err(format!("{source:?} not found in mounted DMG"));
    }
    // `cp -R` into an existing directory produces SuprimSQL.app/SuprimSQL.app,
    // so remove the old bundle first. macOS does not let a running process's
    // bundle be removed, but `/Applications/SuprimSQL.app` on disk is just
    // the read-only template; the running binary has been mapped into memory
    // already and survives.
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|e| format!("remove old app: {e}"))?;
    }
    let status = Command::new("cp")
        .arg("-R")
        .arg(&source)
        .arg(dest)
        .status()
        .map_err(|e| format!("spawn cp: {e}"))?;
    if !status.success() {
        return Err(format!("cp -R exited with {status}"));
    }
    Ok(())
}

fn unmount_dmg(mount_point: &Path) -> Result<(), String> {
    let status = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(mount_point)
        .status()
        .map_err(|e| format!("spawn hdiutil detach: {e}"))?;
    if !status.success() {
        return Err(format!("hdiutil detach exited with {status}"));
    }
    Ok(())
}

fn relaunch() -> Result<(), String> {
    // `-n` forces a new instance even if another SuprimSQL.app is running
    // (which it will be — us). macOS then schedules the new process and our
    // caller calls exit() to release the window slot.
    let status = Command::new("open")
        .args(["-n", APP_INSTALL_PATH])
        .status()
        .map_err(|e| format!("spawn open: {e}"))?;
    if !status.success() {
        return Err(format!("open exited with {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::{NamedTempFile, TempDir};
    use wiremock::matchers::{method, path as wm_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_release() -> LatestRelease {
        LatestRelease {
            version: "9.9.9".to_owned(),
            channel: "stable".to_owned(),
            os: "macos".to_owned(),
            arch: "universal".to_owned(),
            download_url: String::new(), // set per-test
            sha256: String::new(),       // set per-test
            size_bytes: 0,
            release_notes: None,
            release_url: None,
        }
    }

    // ── hex_encode ──────────────────────────────────────────────────────

    #[test]
    fn hex_encode_produces_lowercase_with_leading_zeros() {
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_encode(&[0x00]), "00");
        assert_eq!(hex_encode(&[0x0f, 0xff]), "0fff");
        assert_eq!(hex_encode(&[0xca, 0xfe, 0xba, 0xbe]), "cafebabe");
    }

    #[test]
    fn hex_encode_never_panics_on_full_byte_range() {
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        let hex = hex_encode(&all_bytes);
        assert_eq!(hex.len(), all_bytes.len() * 2);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── verify_sha256 ───────────────────────────────────────────────────

    #[test]
    fn verify_sha256_accepts_matching_digest() {
        // sha256("hello world\n")
        // = a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world\n").unwrap();
        file.flush().unwrap();

        verify_sha256(
            file.path(),
            "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447",
        )
        .expect("matching hash should verify");
    }

    #[test]
    fn verify_sha256_is_case_insensitive() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world\n").unwrap();
        file.flush().unwrap();

        verify_sha256(
            file.path(),
            "A948904F2F0F479B8F8197694B30184B0D2ED1C1CD2A1EC0FB85D299A192A447",
        )
        .expect("upper-case hex should still verify");
    }

    #[test]
    fn verify_sha256_deletes_file_on_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"payload").unwrap();
        file.flush().unwrap();
        let path = file.path().to_owned();
        let _ = file.keep();

        let err = verify_sha256(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect_err("mismatch should error");
        assert!(err.contains("checksum mismatch"));
        assert!(!path.exists(), "mismatched file should be removed");
    }

    #[test]
    fn verify_sha256_errors_cleanly_when_file_missing() {
        let err = verify_sha256(
            Path::new("/nonexistent/path/to/file.dmg"),
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect_err("missing file must error");
        assert!(err.starts_with("read "), "got: {err}");
    }

    // ── cache_path_in ───────────────────────────────────────────────────

    #[test]
    fn cache_path_in_creates_nested_directories() {
        let tmp = TempDir::new().unwrap();
        let path = cache_path_in(tmp.path(), "1.2.3").unwrap();
        assert_eq!(path.file_name().unwrap(), "SuprimSQL-1.2.3.dmg");
        assert!(
            path.parent().unwrap().is_dir(),
            "cache_path_in must create the SuprimSQL/updates/ parent chain"
        );
    }

    #[test]
    fn cache_path_in_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let a = cache_path_in(tmp.path(), "1.0.0").unwrap();
        let b = cache_path_in(tmp.path(), "1.0.0").unwrap();
        assert_eq!(a, b);
    }

    // ── parse_mount_point ───────────────────────────────────────────────

    #[test]
    fn parse_mount_point_extracts_from_canonical_hdiutil_output() {
        // Trimmed version of real `hdiutil attach -plist` output.
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>system-entities</key>
    <array>
        <dict>
            <key>content-hint</key><string>Apple_partition_scheme</string>
            <key>dev-entry</key><string>/dev/disk4</string>
        </dict>
        <dict>
            <key>content-hint</key><string>Apple_HFS</string>
            <key>mount-point</key><string>/Volumes/SuprimSQL 0.1.2</string>
        </dict>
    </array>
</dict>
</plist>"#;
        assert_eq!(
            parse_mount_point(plist).as_deref(),
            Some("/Volumes/SuprimSQL 0.1.2")
        );
    }

    #[test]
    fn parse_mount_point_returns_none_for_empty_output() {
        assert!(parse_mount_point("").is_none());
    }

    #[test]
    fn parse_mount_point_returns_none_when_key_missing() {
        // plist without a mount-point key (detach-only responses do this).
        let plist = r#"<plist><dict><key>other</key><string>x</string></dict></plist>"#;
        assert!(parse_mount_point(plist).is_none());
    }

    #[test]
    fn parse_mount_point_picks_first_occurrence() {
        // Defensive: if a future hdiutil emits multiple, take the first —
        // that's the volume SuprimSQL.app lives in.
        let plist = r#"<key>mount-point</key><string>/Volumes/A</string>
                       <key>mount-point</key><string>/Volumes/B</string>"#;
        assert_eq!(parse_mount_point(plist).as_deref(), Some("/Volumes/A"));
    }

    // ── copy_app_to ─────────────────────────────────────────────────────

    #[test]
    fn copy_app_to_copies_bundled_app_into_dest() {
        let tmp = TempDir::new().unwrap();
        let mount = tmp.path().join("mount");
        let bundle = mount.join(BUNDLED_APP_NAME);
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), b"<plist/>").unwrap();

        let dest = tmp.path().join("Applications").join("SuprimSQL.app");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();

        copy_app_to(&mount, &dest).expect("copy should succeed");

        assert!(dest.is_dir(), "dest must be a directory");
        assert!(dest.join("Contents/Info.plist").exists());
        // `cp -R` must NOT produce SuprimSQL.app/SuprimSQL.app.
        assert!(!dest.join(BUNDLED_APP_NAME).exists());
    }

    #[test]
    fn copy_app_to_removes_existing_bundle_before_copy() {
        let tmp = TempDir::new().unwrap();
        let mount = tmp.path().join("mount");
        std::fs::create_dir_all(mount.join(BUNDLED_APP_NAME).join("Contents")).unwrap();
        std::fs::write(
            mount.join(BUNDLED_APP_NAME).join("Contents/version.txt"),
            b"new",
        )
        .unwrap();

        let dest = tmp.path().join("SuprimSQL.app");
        std::fs::create_dir_all(dest.join("Contents")).unwrap();
        std::fs::write(dest.join("Contents/version.txt"), b"old").unwrap();
        std::fs::write(dest.join("Contents/leftover.txt"), b"x").unwrap();

        copy_app_to(&mount, &dest).unwrap();

        // Leftover from old bundle must be gone (remove_dir_all then cp).
        assert!(!dest.join("Contents/leftover.txt").exists());
        let content = std::fs::read_to_string(dest.join("Contents/version.txt")).unwrap();
        assert_eq!(content, "new");
    }

    #[test]
    fn copy_app_to_errors_when_source_missing() {
        let tmp = TempDir::new().unwrap();
        let err = copy_app_to(tmp.path(), &tmp.path().join("dest.app"))
            .expect_err("empty mount must fail");
        assert!(err.contains("not found in mounted DMG"), "got: {err}");
    }

    // ── stream_to_file ──────────────────────────────────────────────────

    #[tokio::test]
    async fn stream_to_file_writes_body_to_disk() {
        let server = MockServer::start().await;
        let payload = b"hello self-update";
        Mock::given(method("GET"))
            .and(wm_path("/app.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.to_vec()))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");

        stream_to_file(
            &format!("{}/app.dmg", server.uri()),
            &dest,
            payload.len() as u64,
            |_, _| {},
        )
        .await
        .unwrap();

        let written = std::fs::read(&dest).unwrap();
        assert_eq!(written, payload);
    }

    #[tokio::test]
    async fn stream_to_file_reports_monotonic_progress() {
        let server = MockServer::start().await;
        // 100 bytes of filler so the chunks matter.
        let payload = vec![0x5a; 100];
        Mock::given(method("GET"))
            .and(wm_path("/app.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");
        let progress = Arc::new(Mutex::new(Vec::<u64>::new()));
        let progress_ref = progress.clone();

        stream_to_file(
            &format!("{}/app.dmg", server.uri()),
            &dest,
            payload.len() as u64,
            move |done, _| {
                progress_ref.lock().unwrap().push(done);
            },
        )
        .await
        .unwrap();

        let log = progress.lock().unwrap().clone();
        assert!(!log.is_empty(), "on_progress must fire at least once");
        assert!(
            log.windows(2).all(|w| w[0] <= w[1]),
            "progress must be monotonically non-decreasing: {log:?}"
        );
        assert_eq!(
            *log.last().unwrap(),
            payload.len() as u64,
            "final progress must equal payload size"
        );
    }

    #[tokio::test]
    async fn stream_to_file_surfaces_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wm_path("/missing.dmg"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");

        let err = stream_to_file(
            &format!("{}/missing.dmg", server.uri()),
            &dest,
            0,
            |_, _| {},
        )
        .await
        .expect_err("404 must propagate");
        assert!(err.contains("HTTP status"), "got: {err}");
    }

    // ── download_dmg (state + progress wiring) ──────────────────────────

    #[tokio::test]
    async fn download_dmg_publishes_downloading_state_transitions() {
        let server = MockServer::start().await;
        let payload = b"fake dmg bytes";
        Mock::given(method("GET"))
            .and(wm_path("/app.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.to_vec()))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");

        let mut release = sample_release();
        release.download_url = format!("{}/app.dmg", server.uri());
        release.size_bytes = payload.len() as u64;

        let state: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        download_dmg(&state, &release, &dest).await.unwrap();

        // Final state must still be a Downloading progress with bytes_done
        // == payload length (the orchestrator transitions to Verifying
        // afterwards; download_dmg itself does not).
        let guard = state.lock().unwrap();
        match &*guard {
            UpdateState::Installing { progress, .. } => match progress {
                UpdateProgress::Downloading { bytes_done, .. } => {
                    assert_eq!(*bytes_done, payload.len() as u64);
                }
                other => panic!("expected Downloading, got {other:?}"),
            },
            other => panic!("expected Installing, got {other:?}"),
        }
    }

    // ── install_inner end-to-end ────────────────────────────────────────

    #[tokio::test]
    async fn install_inner_fails_on_sha256_mismatch_before_mounting() {
        let server = MockServer::start().await;
        let payload = b"not a real dmg at all";
        Mock::given(method("GET"))
            .and(wm_path("/app.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.to_vec()))
            .mount(&server)
            .await;

        let mut release = sample_release();
        release.download_url = format!("{}/app.dmg", server.uri());
        release.size_bytes = payload.len() as u64;
        // Deliberate mismatch — hash of "not a real dmg..." is anything but this.
        release.sha256 =
            "deadbeef0000000000000000000000000000000000000000000000000000feed".to_owned();

        let state: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        let err = install_inner(&state, &release)
            .await
            .expect_err("bad hash must fail install");
        assert!(
            err.contains("checksum mismatch"),
            "expected checksum error, got: {err}"
        );
    }

    #[tokio::test]
    async fn install_inner_propagates_download_failure() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wm_path("/app.dmg"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut release = sample_release();
        release.download_url = format!("{}/app.dmg", server.uri());
        release.size_bytes = 1;
        release.sha256 = "a".repeat(64);

        let state: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        let err = install_inner(&state, &release)
            .await
            .expect_err("5xx must fail install");
        // The error bubbles up from stream_to_file.
        assert!(err.contains("HTTP status"), "got: {err}");
    }
}
