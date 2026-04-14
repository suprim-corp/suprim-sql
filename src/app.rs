use eframe::egui;
use suprim_sql::db::driver::{DbCommand, DbEvent};
use suprim_sql::storage::{AppConfig, QueryHistoryStore, WorkspaceState};
use tokio::sync::mpsc;

use crate::ui::{
    ConnectionDialog, DeleteConnectionDialog, InputDialog, Sidebar, StatusBar, TabManager,
};

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
    pub(crate) structure_sync_dialog:
        Option<crate::ui::dialog::tool::structure_sync::StructureSyncDialog>,

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

    /// Native macOS menu bar channel + retained handler objects.
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: crate::ui::macos_menu::NativeMenu,
}

impl App {
    pub fn with_channels(
        cc: &eframe::CreationContext<'_>,
        cmd_tx: mpsc::Sender<DbCommand>,
        event_rx: mpsc::Receiver<DbEvent>,
        #[cfg(target_os = "macos")] native_menu: crate::ui::macos_menu::NativeMenu,
    ) -> Self {
        // Register Phosphor icon font so all UI components can use it.
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

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
