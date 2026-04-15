mod app;
mod app_ui;
mod dialog_handlers;
mod event_handler;
mod sidebar_action_handler;
mod ui;

use eframe::egui;
use std::sync::Arc;
use suprim_core::db::worker::DbWorker;
use suprim_core::premium;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Build a tokio multi-thread runtime that lives for the entire process.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    // Keep the runtime alive for the whole app lifetime.
    let _rt_guard = rt.enter();

    // Create the premium gate (real license manager or free stub).
    #[cfg(feature = "premium")]
    let mut gate: Box<dyn premium::PremiumGate> = Box::new(suprim_premium::PremiumLicense::load());
    #[cfg(not(feature = "premium"))]
    let mut gate = premium::create_free_gate();
    if gate.needs_validation() {
        let validate_result = rt.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                gate.validate_online(),
            )
            .await
        });
        match validate_result {
            Ok(Ok(_)) => tracing::info!("License validated: {}", gate.tier_name()),
            Ok(Err(e)) => tracing::warn!("License validation failed: {}", e),
            Err(_) => tracing::warn!("License validation timed out (offline grace applies)"),
        }
    }
    tracing::info!("Running as {} tier", gate.tier_name());
    let gate: Arc<dyn premium::PremiumGate> = Arc::from(gate);

    // Spawn the DB worker inside the runtime.
    let (cmd_tx, event_rx) = DbWorker::spawn(32, 64, Arc::clone(&gate));

    // Load app icon from embedded PNG bytes.
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icons/icon.png"))
        .expect("Failed to decode app icon");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("SuprimSQL")
        .with_icon(icon)
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([800.0, 500.0]);

    // macOS: content extends behind title bar; we render a custom one.
    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "SuprimSQL",
        native_options,
        Box::new(move |cc| {
            // Install native macOS menu bar (replaces winit default).
            #[cfg(target_os = "macos")]
            let native_menu = ui::macos_menu::install_native_menu();

            Ok(Box::new(app::App::with_channels(
                cc,
                cmd_tx,
                event_rx,
                gate,
                #[cfg(target_os = "macos")]
                native_menu,
            )))
        }),
    )
    .unwrap_or_else(|e| eprintln!("Failed to start: {e}"));
}
