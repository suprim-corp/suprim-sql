use eframe::egui;
use std::sync::Arc;
use suprim_core::db::commands::{DbCommand, DbEvent};
use suprim_core::premium::PremiumGate;
use suprim_core::storage::{AppConfig, QueryHistoryStore, WorkspaceState};
use tokio::sync::mpsc;

use crate::ui::{
    ConnectionDialog, DeleteConnectionDialog, InputDialog, LicenseDialog, Sidebar, StatusBar,
    TabManager, UpgradePrompt,
};

/// Re-export from the export module for convenience.
pub use crate::ui::export::PendingExport;

/// Main application state — owned by the eframe runtime on the UI thread.
pub struct App {
    /// Sender to the background DbWorker task.
    pub(crate) cmd_tx: mpsc::Sender<DbCommand>,
    /// Receiver for events coming back from the DbWorker.
    pub(crate) event_rx: mpsc::Receiver<DbEvent>,

    pub(crate) sidebar: Sidebar,
    pub(crate) tab_manager: TabManager,
    pub(crate) statusbar: StatusBar,

    /// Optional modal dialog currently shown.
    pub(crate) connection_dialog: Option<ConnectionDialog>,

    /// Status message shown in the status bar.
    pub(crate) status: String,

    /// Persisted app configuration (connections list).
    pub(crate) config: AppConfig,

    /// Whether the About dialog is open.
    pub(crate) show_about: bool,

    /// Structure Synchronization dialog (None = closed).
    pub(crate) structure_sync_dialog: Option<Box<dyn suprim_core::sync_types::ToolDialog>>,

    /// Delete connection confirmation dialog (None = closed).
    pub(crate) delete_connection_dialog: Option<DeleteConnectionDialog>,
    /// Connection id pending deletion (set when confirm dialog is shown).
    pub(crate) pending_delete_conn: Option<uuid::Uuid>,

    /// Input dialog for New Database / New Schema (None = closed).
    pub(crate) input_dialog: Option<InputDialog>,

    /// Query history store — persisted to disk.
    pub(crate) history: QueryHistoryStore,

    /// Whether the query history panel is open.
    pub(crate) show_history: bool,

    /// Search query for the history panel.
    pub(crate) history_search: String,

    /// Connection IDs to auto-reconnect from saved workspace.
    pub(crate) restore_connected_ids: Vec<uuid::Uuid>,

    /// Premium gate — feature gating (free vs premium).
    pub(crate) gate: Arc<dyn PremiumGate>,

    /// License activation dialog (None = closed).
    pub(crate) license_dialog: Option<LicenseDialog>,

    /// Upgrade prompt dialog (None = closed).
    pub(crate) upgrade_prompt: Option<UpgradePrompt>,

    /// Export dialog (None = closed).
    pub(crate) export_dialog: Option<crate::ui::ExportDialog>,

    /// Pending exports awaiting query results — keyed by synthetic tab_id.
    pub(crate) pending_exports: std::collections::HashMap<uuid::Uuid, crate::app::PendingExport>,

    /// Self-update state — polled on startup, surfaced via a banner in the
    /// status bar when a newer release is available.
    pub(crate) update_state: crate::update::state::SharedUpdateState,

    /// Native macOS menu bar channel + retained handler objects.
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: crate::ui::macos_menu::NativeMenu,
}

impl App {
    pub fn with_channels(
        cc: &eframe::CreationContext<'_>,
        cmd_tx: mpsc::Sender<DbCommand>,
        event_rx: mpsc::Receiver<DbEvent>,
        license: Arc<dyn PremiumGate>,
        #[cfg(target_os = "macos")] native_menu: crate::ui::macos_menu::NativeMenu,
    ) -> Self {
        // Register Phosphor icon font so all UI components can use it.
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        // Add iconflow icon fonts (Devicon + Tabler + Phosphor via iconflow)
        crate::ui::icons::install_fonts(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);

        // Snappier tooltips — default 0.5s feels sluggish, 0.25s keeps
        // accidental hovers from flashing tooltips while still being instant
        // enough to feel responsive.
        cc.egui_ctx.global_style_mut(|style| {
            style.interaction.tooltip_delay = 0.25;
        });

        // Load saved connections from disk.
        let config = AppConfig::load();
        let history = QueryHistoryStore::load();

        let mut sidebar = Sidebar::new();
        sidebar.init_from_config(&config.connections);

        // Restore workspace (open tabs) from last session.
        let workspace = WorkspaceState::load();
        let mut tab_manager = TabManager::new();
        let restore_active = workspace.active_tab;
        let connected_ids = workspace.connected_ids.clone();
        let show_history = workspace.show_history;
        tab_manager.restore_from(workspace.tabs, restore_active);

        Self {
            cmd_tx,
            event_rx,
            sidebar,
            tab_manager,
            statusbar: StatusBar::new(),
            connection_dialog: None,
            status: "Ready".to_string(),
            config,
            show_about: false,
            structure_sync_dialog: None,
            delete_connection_dialog: None,
            pending_delete_conn: None,
            input_dialog: None,
            history,
            show_history,
            history_search: String::new(),
            restore_connected_ids: connected_ids,
            gate: license,
            license_dialog: None,
            upgrade_prompt: None,
            export_dialog: None,
            pending_exports: std::collections::HashMap::new(),
            update_state: spawn_update_check(cc.egui_ctx.clone()),
            #[cfg(target_os = "macos")]
            native_menu,
        }
    }

    /// Save current workspace state (tabs + UI preferences) to disk.
    pub(crate) fn save_workspace(&self) {
        let (tabs, active_tab) = self.tab_manager.snapshot();
        let connected_ids: Vec<uuid::Uuid> = self.sidebar.connected_ids();
        let ws = WorkspaceState {
            tabs,
            active_tab,
            connected_ids,
            show_history: self.show_history,
        };
        ws.save();
    }

    /// Open the account dialog — shows account info if signed in, sign-in form otherwise.
    pub(crate) fn open_license_dialog(&mut self) {
        let tier = self.gate.tier_name().to_string();
        let email = self.gate.user_email().map(|s| s.to_string());
        if let Some(email) = email.as_deref() {
            self.license_dialog = Some(crate::ui::LicenseDialog::with_info(&tier, Some(email)));
        } else {
            self.license_dialog = Some(crate::ui::LicenseDialog::new(&tier));
        }
    }
}

impl eframe::App for App {
    /// Process DB events (called before rendering each frame).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-reconnect connections from saved workspace (first frame only).
        if !self.restore_connected_ids.is_empty() {
            let ids = std::mem::take(&mut self.restore_connected_ids);
            for conn_id in ids {
                if let Some(cfg) = self.config.connections.iter().find(|c| c.id == conn_id) {
                    tracing::info!("Workspace restore: reconnecting {}", cfg.name);
                    self.sidebar.on_connecting(conn_id);
                    self.sidebar.mark_needs_expand(conn_id);
                    let _ = self.cmd_tx.try_send(DbCommand::Connect {
                        config: cfg.clone(),
                    });
                }
            }
            ctx.request_repaint();
        }

        let had_events = self.process_events();
        if had_events {
            ctx.request_repaint();
        }

        #[cfg(target_os = "macos")]
        self.process_menu_actions(ctx);

        // Poll for DB responses at 30fps while any tab is loading.
        if self.tab_manager.any_tab_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui);
    }

    fn on_exit(&mut self) {
        self.save_workspace();
    }
}

/// Spawn an async task that polls the update feed once and stores the result
/// in the shared state. Called at startup; the banner UI polls the state on
/// every frame and shows a message when a newer release is available.
///
/// Calling `ctx.request_repaint()` at the end forces egui to redraw once the
/// result lands, so the banner appears without the user having to move the
/// mouse.
fn spawn_update_check(ctx: egui::Context) -> crate::update::state::SharedUpdateState {
    use crate::update::state::{set, SharedUpdateState};
    use crate::update::{check_for_update, UpdateState};

    let state: SharedUpdateState = std::sync::Arc::new(std::sync::Mutex::new(UpdateState::Idle));
    let state_clone = state.clone();

    // The tokio runtime was installed by main.rs via #[tokio::main]; we just
    // fire-and-forget on it.
    tokio::spawn(async move {
        set(&state_clone, UpdateState::Checking);
        ctx.request_repaint();

        let (os, arch) = platform_triple();
        match check_for_update(os, arch).await {
            Ok(Some(release)) => {
                tracing::info!(version = %release.version, "update available");
                set(&state_clone, UpdateState::Available(release));
            }
            Ok(None) => {
                tracing::debug!("already up to date");
                set(&state_clone, UpdateState::UpToDate);
            }
            Err(e) => {
                tracing::warn!(error = %e, "update check failed");
                set(&state_clone, UpdateState::Failed(e.to_string()));
            }
        }
        ctx.request_repaint();
    });

    state
}

/// Map the compile-time target into the `(os, arch)` tuple the feed expects.
fn platform_triple() -> (&'static str, &'static str) {
    #[cfg(target_os = "macos")]
    let os = "macos";
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";

    // SuprimSQL on macOS ships a single universal binary today; non-macOS
    // reports the actual CPU arch so the server can pick the right asset
    // when those builds exist.
    #[cfg(target_os = "macos")]
    let arch = "universal";
    #[cfg(not(target_os = "macos"))]
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64" // reasonable default
    };

    (os, arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_triple_matches_compile_target() {
        // We can only verify this works for the target the test compiles
        // under. On every other target the constants are cfg'd out, so this
        // test implicitly confirms the cfg matrix routes to SOMETHING for
        // each platform (absence of compile error == coverage).
        let (os, arch) = platform_triple();
        assert!(!os.is_empty());
        assert!(!arch.is_empty());

        #[cfg(target_os = "macos")]
        {
            assert_eq!(os, "macos");
            assert_eq!(arch, "universal");
        }
        #[cfg(target_os = "linux")]
        assert_eq!(os, "linux");
        #[cfg(target_os = "windows")]
        assert_eq!(os, "windows");
    }

    #[test]
    fn platform_arch_is_one_of_known_values() {
        let (_os, arch) = platform_triple();
        assert!(
            matches!(arch, "universal" | "x86_64" | "aarch64"),
            "arch must stay in the server's enum: got {arch}"
        );
    }
}
