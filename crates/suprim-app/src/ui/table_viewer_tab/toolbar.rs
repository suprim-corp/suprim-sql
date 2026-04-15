/// Table viewer toolbar — icon buttons for reload, add, delete, undo, execute.
/// Extracted from `filter_bar.rs` to keep filter input logic separate from toolbar icons.
use eframe::egui;
use suprim_sql::db::commands::DbCommand;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::new_row_editor::NewRowEditor;
use super::TableViewerTab;

impl TableViewerTab {
    /// Render the toolbar icon group (reload, +, −, undo, execute).
    /// Called inside the filter bar's horizontal layout.
    pub(super) fn render_toolbar_icons(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
        icon_color: egui::Color32,
        icon_size: f32,
    ) {
        // Reload / spinner
        if self.is_loading {
            ui.spinner();
        } else {
            let reload_resp = ui.add(
                egui::Label::new(
                    egui::RichText::new(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE)
                        .color(icon_color)
                        .size(icon_size),
                )
                .selectable(false)
                .sense(egui::Sense::click()),
            );
            if reload_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if reload_resp.clicked() {
                self.page = 0;
                self.load(tab_id, cmd_tx);
            }
            reload_resp.on_hover_text("Reload Data");
        }

        // Add Row (+)
        let add_resp = ui.add(
            egui::Label::new(
                egui::RichText::new(egui_phosphor::regular::PLUS)
                    .color(icon_color)
                    .size(icon_size),
            )
            .selectable(false)
            .sense(egui::Sense::click()),
        );
        if add_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if add_resp.clicked() {
            if let Some(result) = &self.result {
                self.new_row_editor = Some(NewRowEditor::new(result.columns.clone()));
            }
        }
        add_resp.on_hover_text("Add Row");

        // Delete Row (−)
        let has_selection = self.selected_cell.is_some() || self.selected_row.is_some();
        let del_resp = ui.add(
            egui::Label::new(
                egui::RichText::new(egui_phosphor::regular::MINUS)
                    .color(if has_selection {
                        icon_color
                    } else {
                        ui.visuals().noninteractive().bg_stroke.color
                    })
                    .size(icon_size),
            )
            .selectable(false)
            .sense(if has_selection {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            }),
        );
        if has_selection && del_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if has_selection && del_resp.clicked() {
            self.pending_toolbar_delete = true;
        }
        del_resp.on_hover_text(if has_selection {
            "Delete Selected Row"
        } else {
            "Delete Row (select a cell first)"
        });

        // Undo (↶) — revert last pending change
        let has_undo = !self.pending.undo_stack.is_empty();
        let undo_resp = ui.add(
            egui::Label::new(
                egui::RichText::new(egui_phosphor::regular::ARROW_U_UP_LEFT)
                    .color(if has_undo {
                        icon_color
                    } else {
                        ui.visuals().noninteractive().bg_stroke.color
                    })
                    .size(icon_size),
            )
            .selectable(false)
            .sense(if has_undo {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            }),
        );
        if has_undo && undo_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if has_undo && undo_resp.clicked() {
            self.pending_undo = true;
        }
        undo_resp.on_hover_text(if has_undo {
            "Undo Last Edit"
        } else {
            "Undo (no edits to undo)"
        });

        // Execute (▲) — commit all pending changes to database
        let has_pending = self.pending.has_changes();
        let change_count = self.pending.change_count();
        let exec_resp = ui.add(
            egui::Label::new(
                egui::RichText::new(egui_phosphor::regular::ARROW_FAT_UP)
                    .color(if has_pending {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else {
                        icon_color
                    })
                    .size(icon_size),
            )
            .selectable(false)
            .sense(egui::Sense::click()),
        );
        if exec_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if exec_resp.clicked() {
            if has_pending {
                self.pending_execute = true;
            } else {
                // No pending changes: just reload
                self.page = 0;
                self.load(tab_id, cmd_tx);
            }
        }
        exec_resp.on_hover_text(if has_pending {
            format!("Execute {} pending change(s)", change_count)
        } else {
            "Execute / Apply Filters".to_string()
        });

        // Pending change count badge
        if has_pending {
            ui.label(
                egui::RichText::new(format!("{}", change_count))
                    .color(egui::Color32::from_rgb(100, 200, 100))
                    .small()
                    .strong(),
            );
        }
    }
}
