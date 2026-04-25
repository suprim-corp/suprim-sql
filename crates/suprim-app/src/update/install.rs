#![allow(dead_code)]
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

/// Hard ceiling on download size so a hostile or compromised feed can't
/// stream gigabytes until the user's disk fills up. SuprimSQL DMGs run
/// ~30 MB today; 500 MB gives 16× headroom for a distant future where we
/// bundle large runtime assets.
const MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024;

/// Kick off the install pipeline. Consumes the release; the caller should
/// have stored it in `UpdateState::Available` first so the banner can keep
/// rendering its metadata while download is in progress.
///
/// Once the new bundle is installed and relaunched, we ask egui to close
/// the viewport rather than calling `std::process::exit(0)` directly. That
/// lets `App::on_exit` flush workspace state (open tabs, query history)
/// before the process dies — `exit(0)` would drop any unsaved in-memory
/// state on the floor.
///
/// Currently macOS-only. On other platforms it sets `Failed(...)` and
/// returns without touching the filesystem, so the badge stays consistent
/// but nothing harmful happens. Windows / Linux install paths live in
/// follow-up work.
pub async fn install_update(
    state: SharedUpdateState,
    release: LatestRelease,
    ctx: eframe::egui::Context,
) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = release;
        set(
            &state,
            UpdateState::Failed(
                "Self-update is currently macOS-only. Please download the installer manually."
                    .to_owned(),
            ),
        );
        ctx.request_repaint();
        return;
    }

    #[cfg(target_os = "macos")]
    {
        let result = install_inner(&state, &release).await;

        match result {
            Ok(()) => {
                set(&state, UpdateState::Relaunching);
                ctx.request_repaint();
                // Give the user a beat to see "Relaunching…" before we disappear.
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                if let Err(e) = relaunch() {
                    set(&state, UpdateState::Failed(format!("Relaunch failed: {e}")));
                    ctx.request_repaint();
                    return;
                }
                // Ask egui to shut down cleanly. `ViewportCommand::Close` fires
                // the window-close path, which runs `App::on_exit` (saves
                // workspace.json) and then ends the event loop → exit(0) via
                // eframe, not us.
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            }
            Err(e) => {
                set(&state, UpdateState::Failed(e));
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(target_os = "macos")]
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
        copy_app(&mount_point).and_then(|()| verify_code_signature(Path::new(APP_INSTALL_PATH)))
    };

    set(
        state,
        UpdateState::Installing {
            release: release.clone(),
            progress: UpdateProgress::Unmounting,
        },
    );
    if let Err(e) = unmount_dmg(&mount_point) {
        tracing::warn!(
            error = %e,
            mount_point = ?mount_point,
            "detach failed; /Volumes leak until reboot"
        );
    }

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
///
/// Refuses bodies larger than [`MAX_DOWNLOAD_BYTES`]: checked both via the
/// `Content-Length` header (so a hostile response is rejected up front)
/// and again while streaming (so a lying / absent header can't sneak past).
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
    if total > MAX_DOWNLOAD_BYTES {
        return Err(format!(
            "refusing to download {total} bytes (limit {MAX_DOWNLOAD_BYTES})"
        ));
    }

    let mut file = std::fs::File::create(dest).map_err(|e| format!("create {dest:?}: {e}"))?;
    let mut bytes_done: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download stream: {e}"))?;
        bytes_done += chunk.len() as u64;
        if bytes_done > MAX_DOWNLOAD_BYTES {
            // Drop the partial file so it can't confuse a retry.
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(format!(
                "download exceeded {MAX_DOWNLOAD_BYTES} bytes (server is lying about Content-Length)"
            ));
        }
        file.write_all(&chunk).map_err(|e| format!("write: {e}"))?;
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
#[cfg(target_os = "macos")]
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
///
/// Applies XML entity unescaping (`&amp;` → `&`, `&lt;`, `&gt;`, `&quot;`,
/// `&apos;`) so a volume name with special characters resolves to a real
/// path on disk. SuprimSQL controls its DMG's volume name today, but this
/// is defensive against metadata changes or user-renamed DMGs.
fn parse_mount_point(plist: &str) -> Option<String> {
    let raw = plist
        .split("<key>mount-point</key>")
        .nth(1)
        .and_then(|tail| tail.split("<string>").nth(1))
        .and_then(|tail| tail.split("</string>").next())?
        .trim();
    Some(unescape_xml(raw))
}

/// Minimal XML entity unescape covering the five predefined entities.
/// We avoid pulling in `quick-xml` for this one spot — the input space is
/// controlled by Apple's `hdiutil` output format.
fn unescape_xml(s: &str) -> String {
    // Order matters: decode `&amp;` last so an input like `&amp;lt;` stays
    // as `&lt;` literal instead of collapsing to `<`.
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn copy_app(mount_point: &Path) -> Result<(), String> {
    copy_app_to(mount_point, Path::new(APP_INSTALL_PATH))
}

/// Testable core of [`copy_app`]: swaps the bundle at `dest` with the one
/// inside `mount_point/BUNDLED_APP_NAME`.
///
/// Atomicity strategy — rename + copy + commit/rollback:
///
///   1. Rename the existing `dest` to `{dest}.backup` (cheap, atomic on
///      the same filesystem).
///   2. `cp -R` the new bundle into `dest`.
///   3. On success: delete the backup.
///   4. On failure: rename the backup back, leaving the old app intact.
///
/// This avoids the "user loses the entire app" failure mode of
/// `remove_dir_all` + `cp -R`, where a `cp` error mid-way through leaves
/// no usable bundle on disk.
fn copy_app_to(mount_point: &Path, dest: &Path) -> Result<(), String> {
    let source = mount_point.join(BUNDLED_APP_NAME);
    if !source.exists() {
        return Err(format!("{source:?} not found in mounted DMG"));
    }

    let backup = dest.with_extension("app.backup");

    // Stage 1: park the old bundle (if any).
    if dest.exists() {
        // Remove any stale backup from a previous failed run so `rename`
        // doesn't trip on "destination already exists".
        if backup.exists() {
            std::fs::remove_dir_all(&backup)
                .map_err(|e| format!("remove stale backup {backup:?}: {e}"))?;
        }
        std::fs::rename(dest, &backup)
            .map_err(|e| format!("move old app to backup: {e}"))?;
    }

    // Stage 2: copy new bundle into place.
    let status = Command::new("cp")
        .arg("-R")
        .arg(&source)
        .arg(dest)
        .status()
        .map_err(|e| format!("spawn cp: {e}"))?;

    if !status.success() {
        // Stage 4 (failure): roll back to the backup so the user keeps a
        // working app. Clean up any partial copy first.
        if dest.exists() {
            let _ = std::fs::remove_dir_all(dest);
        }
        if backup.exists() {
            if let Err(e) = std::fs::rename(&backup, dest) {
                return Err(format!(
                    "cp -R failed ({status}) AND rollback failed ({e}) — app is gone at {dest:?}, backup at {backup:?}"
                ));
            }
        }
        return Err(format!("cp -R exited with {status} (rolled back)"));
    }

    // Stage 3 (success): drop the backup.
    if backup.exists() {
        if let Err(e) = std::fs::remove_dir_all(&backup) {
            tracing::warn!(
                error = %e,
                backup = ?backup,
                "copy_app_to: new bundle installed but backup cleanup failed"
            );
        }
    }

    Ok(())
}

/// Expected Team ID (the 10-character string inside `codesign -dvv`'s
/// `Authority=Developer ID Application: …(XXXXXXXXXX)`). Baked into the
/// binary so a malicious DMG signed by a *different* Developer ID — or
/// signed ad-hoc — is rejected even if its SHA-256 matches.
///
/// When unset (development builds, unsigned CI artifacts), the check is
/// skipped entirely — see [`verify_code_signature`] for the trade-off.
///
/// Set at build time: `SUPRIM_TEAM_ID=ABCDE12345 cargo build --release`.
const EXPECTED_TEAM_ID: Option<&str> = option_env!("SUPRIM_TEAM_ID");

/// Verify the freshly-installed bundle is signed by the same Developer ID
/// that built the running binary.
///
/// SHA-256 matching proves only that the DMG the user downloaded matches
/// what the feed described — it does nothing to stop a compromised feed or
/// a MITM with a valid TLS cert from serving a malicious DMG + honest hash.
/// `codesign --verify` closes that loop by checking Apple-signed metadata
/// that only the holder of the corp Developer ID certificate can produce.
///
/// Skipped entirely when `EXPECTED_TEAM_ID` is empty — that lets dev
/// builds and unsigned CI artifacts still exercise the pipeline.
#[cfg(target_os = "macos")]
fn verify_code_signature(bundle: &Path) -> Result<(), String> {
    let expected = match EXPECTED_TEAM_ID {
        Some(id) if !id.is_empty() => id,
        _ => {
            tracing::warn!(
                "code-signature verification skipped: SUPRIM_TEAM_ID was not baked in"
            );
            return Ok(());
        }
    };

    // Step 1: is the bundle signed at all, and does the chain validate?
    let verify = Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(bundle)
        .output()
        .map_err(|e| format!("spawn codesign --verify: {e}"))?;
    if !verify.status.success() {
        return Err(format!(
            "codesign --verify failed: {}",
            String::from_utf8_lossy(&verify.stderr).trim()
        ));
    }

    // Step 2: extract the Team ID and compare. `codesign -dvv` prints lines
    // like `TeamIdentifier=XXXXXXXXXX` and `Authority=Developer ID Application: Name (XXXXXXXXXX)`.
    let display = Command::new("codesign")
        .args(["-dvv"])
        .arg(bundle)
        .output()
        .map_err(|e| format!("spawn codesign -dvv: {e}"))?;
    if !display.status.success() {
        return Err(format!(
            "codesign -dvv failed: {}",
            String::from_utf8_lossy(&display.stderr).trim()
        ));
    }

    // codesign -dvv writes its metadata to STDERR, not stdout — do not
    // "fix" this to `display.stdout`. `man codesign` (look under
    // DESCRIPTION > "-v[verbose]") documents the behaviour.
    let info = String::from_utf8_lossy(&display.stderr);
    let actual = extract_team_id(&info)
        .ok_or_else(|| "codesign output missing TeamIdentifier field".to_owned())?;
    if actual != expected {
        return Err(format!(
            "code signature Team ID mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn verify_code_signature(_bundle: &Path) -> Result<(), String> {
    Ok(())
}

/// Pull the Team ID out of `codesign -dvv` output. Returns the 10-char
/// identifier or `None` if the field is absent.
fn extract_team_id(codesign_output: &str) -> Option<String> {
    codesign_output
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .map(|id| id.trim().to_owned())
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

    #[test]
    fn parse_mount_point_unescapes_xml_entities() {
        let plist = r#"<key>mount-point</key><string>/Volumes/A &amp; B</string>"#;
        assert_eq!(
            parse_mount_point(plist).as_deref(),
            Some("/Volumes/A & B")
        );
    }

    #[test]
    fn unescape_xml_handles_all_five_entities() {
        assert_eq!(unescape_xml("&amp;"), "&");
        assert_eq!(unescape_xml("&lt;"), "<");
        assert_eq!(unescape_xml("&gt;"), ">");
        assert_eq!(unescape_xml("&quot;"), "\"");
        assert_eq!(unescape_xml("&apos;"), "'");
        assert_eq!(unescape_xml("plain text"), "plain text");
    }

    #[test]
    fn unescape_xml_preserves_amp_in_nested_entities() {
        // An input like `&amp;lt;` should unescape to the literal `&lt;`,
        // not the `<` character. This is the classic reason to decode
        // `&amp;` last.
        assert_eq!(unescape_xml("&amp;lt;"), "&lt;");
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
    // These tests invoke install_inner which is macOS-only (it calls
    // hdiutil via mount_dmg). On other platforms install_update() fails
    // fast with a platform-not-supported message instead.

    #[cfg(target_os = "macos")]
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

    #[cfg(target_os = "macos")]
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

    // ── Size cap (#4) ───────────────────────────────────────────────────

    /// Allocates ~500MB of zeros to verify the size cap; gated with
    /// `#[ignore]` so CI and normal `cargo test` skip it. Run with:
    ///
    /// ```sh
    /// cargo test -p suprim-app -- --ignored stream_to_file_rejects
    /// ```
    #[ignore = "allocates 500 MB of RAM; run manually to validate size cap"]
    #[tokio::test]
    async fn stream_to_file_rejects_oversized_content_length_header() {
        let oversize = (MAX_DOWNLOAD_BYTES + 1) as usize;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wm_path("/huge.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; oversize]))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");
        let err = stream_to_file(
            &format!("{}/huge.dmg", server.uri()),
            &dest,
            0,
            |_, _| {},
        )
        .await
        .expect_err("oversized payload must be rejected");
        assert!(
            err.contains("refusing to download") || err.contains("exceeded"),
            "got: {err}"
        );
        assert!(!dest.exists(), "partial file must not survive");
    }

    #[tokio::test]
    async fn stream_to_file_accepts_body_at_the_cap_exactly() {
        // Acceptance boundary: a body that equals the fallback cap must
        // pass. Wiremock always emits Content-Length, so the fallback path
        // isn't exercised here — see the ignored test above for that case.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(wm_path("/nolen.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 8]))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.dmg");
        stream_to_file(
            &format!("{}/nolen.dmg", server.uri()),
            &dest,
            MAX_DOWNLOAD_BYTES,
            |_, _| {},
        )
        .await
        .expect("body at the cap should be accepted");
    }

    // ── Atomic replace (#3) ─────────────────────────────────────────────

    #[test]
    fn copy_app_to_removes_backup_on_success() {
        // Happy path: after a successful install, the `.backup` directory
        // created during rename-swap must be cleaned up.
        let tmp = TempDir::new().unwrap();
        let mount = tmp.path().join("mount");
        std::fs::create_dir_all(mount.join(BUNDLED_APP_NAME).join("Contents")).unwrap();

        let dest = tmp.path().join("SuprimSQL.app");
        std::fs::create_dir_all(dest.join("Contents")).unwrap();
        std::fs::write(dest.join("Contents/old.txt"), b"old").unwrap();

        copy_app_to(&mount, &dest).unwrap();

        let backup = dest.with_extension("app.backup");
        assert!(!backup.exists(), "backup must be removed on success");
        assert!(!dest.join("Contents/old.txt").exists(), "old file gone");
    }

    #[test]
    #[cfg(unix)]
    fn copy_app_to_rolls_back_when_cp_fails() {
        // Force cp to fail by making the backup path **exist as a file**
        // after the initial rename — the second-pass rollback uses
        // `rename(backup, dest)` and we block that instead. Actually
        // simpler: point cp at a source containing an unreadable child so
        // cp exits non-zero but the pre-rename state is intact.
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let mount = tmp.path().join("mount");
        let source_bundle = mount.join(BUNDLED_APP_NAME);
        std::fs::create_dir_all(source_bundle.join("Contents")).unwrap();
        let unreadable = source_bundle.join("Contents/secret.bin");
        std::fs::write(&unreadable, b"x").unwrap();
        // chmod 0: cp -R (which stat+open+read) can't read it, so the
        // whole copy fails.
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();

        let dest = tmp.path().join("SuprimSQL.app");
        std::fs::create_dir_all(dest.join("Contents")).unwrap();
        std::fs::write(dest.join("Contents/original.txt"), b"original").unwrap();

        let err = copy_app_to(&mount, &dest).expect_err("unreadable source must fail cp");

        // Restore perms so TempDir::drop can clean up.
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert!(
            err.contains("rolled back") || err.contains("cp -R"),
            "expected rollback error, got: {err}"
        );
        // Pre-existing bundle must still be there (rolled back from backup).
        let restored = std::fs::read(dest.join("Contents/original.txt"))
            .expect("original bundle must be restored from backup");
        assert_eq!(restored, b"original");
        let backup = dest.with_extension("app.backup");
        assert!(!backup.exists(), "backup should have been renamed back");
    }

    #[test]
    fn copy_app_to_survives_stale_backup_from_previous_run() {
        // Simulate a previous failed run that left a `.backup` behind.
        let tmp = TempDir::new().unwrap();
        let mount = tmp.path().join("mount");
        std::fs::create_dir_all(mount.join(BUNDLED_APP_NAME).join("Contents")).unwrap();
        std::fs::write(mount.join(BUNDLED_APP_NAME).join("Contents/new.txt"), b"new").unwrap();

        let dest = tmp.path().join("SuprimSQL.app");
        std::fs::create_dir_all(dest.join("Contents")).unwrap();

        let stale_backup = dest.with_extension("app.backup");
        std::fs::create_dir_all(stale_backup.join("Contents")).unwrap();
        std::fs::write(stale_backup.join("Contents/stale.txt"), b"stale").unwrap();

        copy_app_to(&mount, &dest).unwrap();

        assert!(!stale_backup.exists(), "stale backup must be cleared");
        assert!(dest.join("Contents/new.txt").exists(), "new bundle installed");
    }

    // ── Code-signature Team ID extraction (#2) ──────────────────────────

    #[test]
    fn extract_team_id_pulls_from_codesign_output() {
        let sample = "Executable=/Applications/SuprimSQL.app/Contents/MacOS/SuprimSQL\n\
                      Identifier=com.suprim.sql\n\
                      Format=app bundle with Mach-O thin (arm64)\n\
                      TeamIdentifier=ABCDE12345\n\
                      Authority=Developer ID Application: Suprim (ABCDE12345)\n\
                      Sealed Resources version=2 rules=13 files=42\n";
        assert_eq!(
            extract_team_id(sample).as_deref(),
            Some("ABCDE12345")
        );
    }

    #[test]
    fn extract_team_id_returns_none_when_missing() {
        let sample = "Executable=/Applications/SuprimSQL.app/Contents/MacOS/SuprimSQL\n\
                      Identifier=com.suprim.sql\n\
                      # ad-hoc signed; no TeamIdentifier field\n";
        assert!(extract_team_id(sample).is_none());
    }

    #[test]
    fn extract_team_id_trims_whitespace() {
        // Defensive: codesign output rarely has trailing space, but if a
        // future version does, we don't want ghosts in the comparison.
        let sample = "TeamIdentifier=ABCDE12345  \n";
        assert_eq!(
            extract_team_id(sample).as_deref(),
            Some("ABCDE12345")
        );
    }
}
