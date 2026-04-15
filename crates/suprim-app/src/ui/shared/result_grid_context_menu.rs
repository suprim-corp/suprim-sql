/// Context-menu rendering for the result grid.
///
/// Extracted from `result_grid.rs` to keep each file under ~200 lines.
use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use super::result_grid::CellAction;

/// Shorthand: button with pointer cursor on hover.
fn btn(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    ui.button(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Shorthand: button with shortcut text and pointer cursor.
fn btn_shortcut(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    shortcut: &str,
) -> egui::Response {
    ui.add(egui::Button::new(label).shortcut_text(shortcut))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// OS-aware modifier key label: "⌘" on macOS, "Ctrl+" on others.
pub(crate) fn mod_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    }
}

/// Shared alias for the cell action reference used in context menus.
pub(crate) type CellActionRef = Rc<RefCell<Option<(CellAction, usize, usize)>>>;

/// Render the context-menu items for a cell.
pub(crate) fn render_cell_context_menu(
    ui: &mut egui::Ui,
    row: usize,
    col: usize,
    is_null: bool,
    action_ref: &CellActionRef,
) {
    let m = mod_key();

    // ── Copy ──
    if btn_shortcut(ui, "Copy", &format!("{m}C")).clicked() {
        *action_ref.borrow_mut() = Some((CellAction::Copy, row, col));
        ui.close();
    }

    ui.menu_button("Copy as", |ui| {
        if btn(ui, "JSON").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsJson, row, col));
            ui.close();
        }
        if btn(ui, "CSV").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsCsv, row, col));
            ui.close();
        }
        if btn(ui, "SQL").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsSql, row, col));
            ui.close();
        }
    });

    // ── Paste ──
    if btn_shortcut(ui, "Paste", &format!("{m}V")).clicked() {
        *action_ref.borrow_mut() = Some((CellAction::Paste, row, col));
        ui.close();
    }

    ui.separator();

    // ── Set Value ──
    ui.menu_button("Set Value", |ui| {
        let null_label = if is_null {
            egui::RichText::new("NULL  ✓")
        } else {
            egui::RichText::new("NULL")
        };
        if btn(ui, null_label).clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetNull, row, col));
            ui.close();
        }
        if btn(ui, "Empty String").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetEmpty, row, col));
            ui.close();
        }
        if btn(ui, "Default").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetDefault, row, col));
            ui.close();
        }
    });

    // ── Edit Value ──
    if btn(ui, "Edit Value...").clicked() {
        *action_ref.borrow_mut() = Some((CellAction::EditValue, row, col));
        ui.close();
    }

    ui.separator();

    // ── Export Results ──
    if btn(ui, "Export Results...").clicked() {
        *action_ref.borrow_mut() = Some((CellAction::ExportResults, row, col));
        ui.close();
    }

    // ── Duplicate Row ──
    if btn_shortcut(ui, "Duplicate", &format!("{m}D")).clicked() {
        *action_ref.borrow_mut() = Some((CellAction::DuplicateRow, row, col));
        ui.close();
    }

    // ── Delete Row ──
    let delete_label = egui::RichText::new("Delete").color(egui::Color32::from_rgb(220, 60, 60));
    if btn_shortcut(ui, delete_label, "⌫").clicked() {
        *action_ref.borrow_mut() = Some((CellAction::DeleteRow, row, col));
        ui.close();
    }
}
