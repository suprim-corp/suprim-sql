//! Query History panel — searchable list of previously executed queries.

mod history_list;

use eframe::egui;
use suprim_core::storage::QueryHistoryStore;

/// Output from the history panel — tells the app what to do.
pub(crate) struct HistoryPanelOutput {
    /// User wants to load this SQL into the active editor.
    pub load_sql: Option<String>,
    /// User wants to clear all history.
    pub clear_all: bool,
    /// User wants to close the panel.
    pub close: bool,
}

/// Render the query history panel.
pub(crate) fn render_history_panel(
    ui: &mut egui::Ui,
    history: &mut QueryHistoryStore,
    search_query: &mut String,
) -> HistoryPanelOutput {
    let mut output = HistoryPanelOutput {
        load_sql: None,
        clear_all: false,
        close: false,
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} Query History",
                egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE
            ))
            .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Close button
            if ui
                .button(egui::RichText::new(egui_phosphor::regular::X).small())
                .on_hover_text("Close panel (⌘Y)")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                output.close = true;
            }
            if ui
                .button(
                    egui::RichText::new(egui_phosphor::regular::TRASH)
                        .color(egui::Color32::from_rgb(200, 60, 60)),
                )
                .on_hover_text("Clear all history")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                output.clear_all = true;
            }
            let search_resp = ui.add(
                egui::TextEdit::singleline(search_query)
                    .hint_text("Search queries...")
                    .desired_width(200.0),
            );
            // Focus search on panel open
            if search_resp.gained_focus() {
                search_resp.request_focus();
            }
        });
    });

    ui.separator();

    let filtered = history.search(search_query);
    history_list::render_history_list(ui, &filtered, &mut output);

    output
}
