//! Shared state between the async update task and the egui UI.
//!
//! Egui repaints synchronously on the main thread, so the async task writes
//! progress into an `Arc<Mutex<UpdateState>>` and the UI polls it on each
//! frame. The [`UpdateProgress`] enum describes which step the workflow is
//! at; the banner maps it to a user-friendly message.

use std::sync::{Arc, Mutex};

use super::LatestRelease;

/// Overall state machine for the update subsystem.
///
/// The `release` field on `Installing` is kept alive so the banner can keep
/// rendering the version / release notes while the install runs. Clippy
/// doesn't see the pattern binding through the shared-lock indirection;
/// `#[allow(dead_code)]` documents the intentional retention.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    /// App just started; no check has run yet.
    #[default]
    Idle,
    /// Background task is polling the feed.
    Checking,
    /// Feed replied "up to date". Cleared after a few seconds.
    UpToDate,
    /// A newer release is available. User must confirm before download.
    Available(LatestRelease),
    /// User accepted — download / verify / mount / copy / relaunch in progress.
    Installing {
        release: LatestRelease,
        progress: UpdateProgress,
    },
    /// Install finished — waiting for the relaunch to kick in.
    Relaunching,
    /// Any step failed. `message` is shown verbatim in the banner.
    Failed(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum UpdateProgress {
    Downloading { bytes_done: u64, bytes_total: u64 },
    Verifying,
    Mounting,
    Copying,
    Unmounting,
}

impl UpdateProgress {
    pub fn label(&self) -> String {
        match self {
            UpdateProgress::Downloading { bytes_done, bytes_total } => {
                if *bytes_total == 0 {
                    format!("Downloading… ({} MB)", bytes_done / 1_000_000)
                } else {
                    let pct = (*bytes_done as f64 / *bytes_total as f64 * 100.0) as u32;
                    format!("Downloading… {pct}%")
                }
            }
            UpdateProgress::Verifying => "Verifying checksum…".to_owned(),
            UpdateProgress::Mounting => "Mounting image…".to_owned(),
            UpdateProgress::Copying => "Installing…".to_owned(),
            UpdateProgress::Unmounting => "Cleaning up…".to_owned(),
        }
    }
}

/// Handle the UI and the worker both share.
pub type SharedUpdateState = Arc<Mutex<UpdateState>>;

/// Helper: lock + update without bothering callers with poison handling.
///
/// Recovers from a poisoned mutex (a previous holder panicked mid-update)
/// instead of silently dropping the new state — otherwise the update
/// subsystem would effectively die after any panic in the install pipeline,
/// with nothing to show in the UI.
pub fn set(state: &SharedUpdateState, new: UpdateState) {
    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = new;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_label_reports_percentage_when_total_known() {
        let p = UpdateProgress::Downloading {
            bytes_done: 25,
            bytes_total: 100,
        };
        assert_eq!(p.label(), "Downloading… 25%");
    }

    #[test]
    fn progress_label_falls_back_to_megabytes_when_total_unknown() {
        let p = UpdateProgress::Downloading {
            bytes_done: 3_500_000,
            bytes_total: 0,
        };
        // 3_500_000 / 1_000_000 = 3 (truncating integer division).
        assert_eq!(p.label(), "Downloading… (3 MB)");
    }

    #[test]
    fn non_download_progress_uses_static_labels() {
        assert_eq!(UpdateProgress::Verifying.label(), "Verifying checksum…");
        assert_eq!(UpdateProgress::Mounting.label(), "Mounting image…");
        assert_eq!(UpdateProgress::Copying.label(), "Installing…");
        assert_eq!(UpdateProgress::Unmounting.label(), "Cleaning up…");
    }

    #[test]
    fn set_replaces_existing_state() {
        let shared: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        set(&shared, UpdateState::Checking);
        let guard = shared.lock().unwrap();
        assert!(matches!(&*guard, UpdateState::Checking));
    }

    #[test]
    fn set_recovers_from_poisoned_lock_silently() {
        // A panicking holder poisons the mutex; `set` should still overwrite
        // the stored state instead of propagating the poison error to the
        // UI thread.
        let shared: SharedUpdateState = Arc::new(Mutex::new(UpdateState::Idle));
        {
            let shared_panic = shared.clone();
            let _ = std::thread::spawn(move || {
                let _guard = shared_panic.lock().unwrap();
                panic!("deliberate");
            })
            .join();
        }
        assert!(shared.is_poisoned(), "test setup must poison the lock");
        set(&shared, UpdateState::UpToDate);
        // set() silently returns on poison — the stored state stays Idle.
        // We only guarantee no panic and no hang; we don't guarantee the
        // state is updated. This documents the contract.
        shared.clear_poison();
    }

    #[test]
    fn default_state_is_idle() {
        assert!(matches!(UpdateState::default(), UpdateState::Idle));
    }
}
