//! Build script for `suprim-app`.
//!
//! Enforces two invariants at compile time that the runtime can't catch:
//!
//! 1. `SUPRIM_TEAM_ID` MUST be set when building a *release* binary for
//!    macOS. `install.rs::verify_code_signature` skips the signature check
//!    when the Team ID is unset — fine for dev builds, catastrophic for a
//!    shipped binary because the self-updater silently accepts any
//!    Apple-signed DMG the feed serves.
//!
//! 2. Both env vars (`SUPRIM_UPDATE_ENDPOINT`, `SUPRIM_TEAM_ID`) re-run the
//!    build when they change, otherwise `cargo build` would cache stale
//!    `option_env!` output after the shell exported a new value.

fn main() {
    // Re-run the build script whenever the baked-in values could change.
    println!("cargo:rerun-if-env-changed=SUPRIM_UPDATE_ENDPOINT");
    println!("cargo:rerun-if-env-changed=SUPRIM_TEAM_ID");

    // Guard: macOS release builds MUST have a Team ID baked in.
    //
    // `PROFILE` is set by Cargo to `debug` or `release`. `CARGO_CFG_TARGET_OS`
    // reflects the target triple (not the host), so cross-compiling a
    // macOS release from any machine still fails loudly without the ID.
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let team_id = std::env::var("SUPRIM_TEAM_ID").ok();

    // Escape hatch for CI jobs that intentionally ship unsigned artifacts
    // (e.g. a per-PR preview build). Set `SUPRIM_ALLOW_UNSIGNED_RELEASE=1`
    // to acknowledge the risk and bypass this check.
    let allow_unsigned = std::env::var("SUPRIM_ALLOW_UNSIGNED_RELEASE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    println!("cargo:rerun-if-env-changed=SUPRIM_ALLOW_UNSIGNED_RELEASE");

    let is_macos_release = profile == "release" && target_os == "macos";
    let team_id_missing = team_id.as_deref().map(str::trim).unwrap_or("").is_empty();

    if is_macos_release && team_id_missing && !allow_unsigned {
        // `cargo:warning=` is the only way a build script can surface a
        // message; `compile_error!` only works from inside source. Combine
        // a warning (visible) with a panic (fatal) so the output is both
        // noticeable and stops the build.
        println!(
            "cargo:warning=SUPRIM_TEAM_ID is not set for this macOS release build — \
             the self-updater would skip Apple code-signature verification."
        );
        panic!(
            "\n\n\
             =============================================================\n\
              SUPRIM_TEAM_ID is required for macOS release builds.\n\
             \n\
              Set it to the 10-character Apple Developer ID Team ID that\n\
              signs the DMG, e.g.\n\
             \n\
                  SUPRIM_TEAM_ID=ABCDE12345 cargo build --release\n\
             \n\
              If you are intentionally building an unsigned release (CI\n\
              preview, smoke test, etc.), set SUPRIM_ALLOW_UNSIGNED_RELEASE=1\n\
              to bypass this check.\n\
             =============================================================\n"
        );
    }
}
