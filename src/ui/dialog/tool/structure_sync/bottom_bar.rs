//! Bottom bar for the Structure Synchronization dialog.
//!
//! Contains Options, Copy Script, Close, Compare/Re-Compare, and navigation buttons.

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
    advance_to_preview: &mut bool,
    back_to_done: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .button("Options")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // TODO: options dialog
        }

        // Copy Script — available in Done and Preview states
        let show_copy = matches!(compare_state, CompareState::Done | CompareState::Preview)
            && !ddl_script.is_empty();

        if show_copy {
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
                    // Next → Preview DDL
                    if ui
                        .button(format!("Next  {}", egui_phosphor::regular::ARROW_RIGHT))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *advance_to_preview = true;
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
                CompareState::Preview => {
                    // Execute (disabled for now — TODO: step 5)
                    let execute_btn =
                        egui::Button::new(format!("Execute  {}", egui_phosphor::regular::PLAY));
                    if ui
                        .add_enabled(false, execute_btn)
                        .on_disabled_hover_text("Execute step coming soon")
                        .clicked()
                    {
                        // TODO: advance to execute step
                    }
                    // Back → return to diff results
                    if ui
                        .button(format!("{}  Back", egui_phosphor::regular::ARROW_LEFT))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *back_to_done = true;
                    }
                }
            }
        });
    });
}
