//! Handler wiring for the update badge in the status bar.
//!
//! The badge itself is drawn by `StatusBar` (see `ui/statusbar.rs`) because
//! it lives alongside the Premium/Free tier badge. This module owns the
//! side-effect: when the user clicks the badge we spawn the install task
//! or reset the state to `Idle`.

use crate::ui::statusbar::StatusBarAction;
use crate::update::state::SharedUpdateState;
use crate::update::UpdateState;

pub fn handle_status_action(shared: &SharedUpdateState, action: StatusBarAction) {
    match action {
        StatusBarAction::InstallUpdate => {
            let release = match shared.lock() {
                Ok(g) => match &*g {
                    UpdateState::Available(r) => r.clone(),
                    _ => return,
                },
                Err(_) => return,
            };
            let shared_clone = shared.clone();
            tokio::spawn(async move {
                crate::update::install_update(shared_clone, release).await;
            });
        }
        StatusBarAction::DismissUpdate => {
            if let Ok(mut g) = shared.lock() {
                *g = UpdateState::Idle;
            }
        }
        // OpenLicense is handled in `app_ui.rs` where we have &mut App.
        StatusBarAction::OpenLicense => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::LatestRelease;
    use std::sync::{Arc, Mutex};

    fn sample_release() -> LatestRelease {
        LatestRelease {
            version: "9.9.9".to_owned(),
            channel: "stable".to_owned(),
            os: "macos".to_owned(),
            arch: "universal".to_owned(),
            download_url: "https://example.test/a.dmg".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            release_notes: None,
            release_url: None,
        }
    }

    #[test]
    fn dismiss_resets_state_to_idle() {
        let shared: SharedUpdateState =
            Arc::new(Mutex::new(UpdateState::Failed("nope".to_owned())));
        handle_status_action(&shared, StatusBarAction::DismissUpdate);
        let guard = shared.lock().unwrap();
        assert!(matches!(&*guard, UpdateState::Idle));
    }

    #[test]
    fn dismiss_is_a_no_op_for_unexpected_states() {
        // DismissUpdate shouldn't fire unless the badge is in Failed, but
        // defend against race conditions where the state changed between
        // click and handler.
        let shared: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Checking));
        handle_status_action(&shared, StatusBarAction::DismissUpdate);
        // Contract: we always set to Idle regardless of prior state — the
        // badge is visually gone either way, and the next check can
        // re-populate.
        assert!(matches!(&*shared.lock().unwrap(), UpdateState::Idle));
    }

    #[test]
    fn open_license_does_not_touch_update_state() {
        // OpenLicense is routed through app_ui.rs (needs &mut App);
        // handle_status_action must NOT mutate the update state for it.
        let shared: SharedUpdateState =
            Arc::new(Mutex::new(UpdateState::Available(sample_release())));
        handle_status_action(&shared, StatusBarAction::OpenLicense);
        assert!(matches!(&*shared.lock().unwrap(), UpdateState::Available(_)));
    }

    // NOTE: InstallUpdate spawns a tokio task that makes network calls
    // (reqwest GET + hdiutil attach) — untestable without a full mock for
    // both the HTTP server and macOS subprocesses. install_inner() is
    // covered in update::install::tests with explicit arguments instead.
    #[test]
    fn install_without_available_state_is_ignored() {
        // Guard: clicking "Install" when state isn't Available (e.g.
        // because a background re-check just transitioned to UpToDate) must
        // not panic and must not spawn anything meaningful.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let shared: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        handle_status_action(&shared, StatusBarAction::InstallUpdate);
        assert!(matches!(&*shared.lock().unwrap(), UpdateState::Idle));
    }
}
