use eframe::egui;
use suprim_sql::db::driver::{DbCommand, DbEvent};
use suprim_sql::storage::AppConfig;
use tokio::sync::mpsc;

use crate::ui::{ConnectionDialog, Sidebar, StatusBar, TabManager};

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

        // Auto-reconnect all saved connections.
        for conn in &config.connections {
            let _ = cmd_tx.try_send(DbCommand::Connect {
                config: conn.clone(),
            });
        }

        Self {
            cmd_tx,
            event_rx,
            sidebar: Sidebar::new(),
            tab_manager: TabManager::new(),
            statusbar: StatusBar::new(),
            connection_dialog: None,
            status: if config.connections.is_empty() {
                "Ready".to_string()
            } else {
                format!(
                    "Reconnecting {} saved connection(s)\u{2026}",
                    config.connections.len()
                )
            },
            config,
            show_about: false,
            #[cfg(target_os = "macos")]
            native_menu,
        }
    }
}

impl eframe::App for App {
    /// Process DB events (called before rendering each frame).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
}
