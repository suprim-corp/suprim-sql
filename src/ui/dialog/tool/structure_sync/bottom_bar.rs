//! Bottom bar for the Structure Synchronization dialog.
//!
//! Contains Options, Copy Script, Close, and Compare/Re-Compare buttons.

use eframe::egui;

use crate::ui::dialog::tool::structure_sync::types::CompareState;

pub(crate) fn render_bottom_bar(
    ui: &mut egui::Ui,
    compare_state: &CompareState,
    ddl_script: &str,
    status: &mut Option<String>,
    open: &mut bool,
    run_compare: &mut bool,
    reset_to_idle: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .button("Options")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // TODO: options dialog
        }

        if *compare_state == CompareState::Done && !ddl_script.is_empty() {
            if ui
                .button(format!(
                    "{}  Copy Script",
                    egui_phosphor::regular::CLIPBOARD_TEXT
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.ctx().copy_text(ddl_script.to_owned());
                *status = Some("Script copied to clipboard".into());
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let compare_enabled = *compare_state != CompareState::Loading;
            let compare_label = match compare_state {
                CompareState::Loading => "Comparing...",
                CompareState::Done => "Re-Compare",
                _ => "Compare",
            };
            let btn_resp = ui
                .add_enabled(compare_enabled, egui::Button::new(compare_label))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if btn_resp.clicked() {
                if *compare_state == CompareState::Done {
                    *reset_to_idle = true;
                } else {
                    *run_compare = true;
                }
            }
            if ui
                .button("Close")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *open = false;
            }
        });
    });
}
