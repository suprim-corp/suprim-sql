mod app;
mod ui;

use eframe::egui;
use suprim_sql::db::worker::DbWorker;

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

    // Spawn the DB worker inside the runtime.
    let (cmd_tx, event_rx) = DbWorker::spawn(32, 64);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("suprim-sql")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "suprim-sql",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::App::with_channels(cc, cmd_tx, event_rx)))),
    )
    .unwrap_or_else(|e| eprintln!("Failed to start: {e}"));
}
