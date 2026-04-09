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
            match compare_state {
                CompareState::Idle => {
                    // Step 1: Close | Compare
                    if ui
                        .button("Compare")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *run_compare = true;
                    }
                    if ui
                        .button(format!("{}  Close", egui_phosphor::regular::X))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *open = false;
                    }
                }
                CompareState::Loading => {
                    ui.add_enabled(false, egui::Button::new("Comparing..."));
                    if ui
                        .button(format!("{}  Close", egui_phosphor::regular::X))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *open = false;
                    }
                }
                CompareState::Done => {
                    // Step 2: Back | Re-Compare | Next
                    if ui
                        .button(format!("Next  {}", egui_phosphor::regular::ARROW_RIGHT))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        // TODO: advance to step 3
                    }
                    if ui
                        .button("Re-Compare")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *run_compare = true;
                    }
                    if ui
                        .button(format!("{}  Back", egui_phosphor::regular::ARROW_LEFT))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *reset_to_idle = true;
                    }
                }
            }
        });
    });
}
