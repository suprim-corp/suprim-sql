//! Tristate checkbox widget + tree node rendering for the export dialog.

use eframe::egui;

use super::types::{ExportDatabaseItem, ExportSchemaItem};

// ── Tree node rendering ─────────────────────────────────────────────────────

pub(super) fn render_database_node(ui: &mut egui::Ui, db: &mut ExportDatabaseItem) {
    let total: usize = db.schemas.iter().map(|s| s.tables.len()).sum();
    let selected: usize = db
        .schemas
        .iter()
        .flat_map(|s| s.tables.iter())
        .filter(|t| t.selected)
        .count();

    let id = ui.make_persistent_id(format!("export_db_{}", db.name));
    let cs =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, db.expanded);

    cs.show_header(ui, |ui| {
        let tri = tristate(selected, total);
        let mut new_checked = tri == TriState::Checked;
        if tristate_checkbox(ui, &mut new_checked, tri).changed() {
            // Mixed/Checked → unselect all; Unchecked → select all
            let target = tri == TriState::Unchecked;
            for s in &mut db.schemas {
                for t in &mut s.tables {
                    t.selected = target;
                }
            }
        }
        ui.label(crate::ui::icons::db::database(
            14.0,
            crate::ui::icons::db::COLOR_DATABASE,
        ));
        ui.label(egui::RichText::new(&db.name).strong());
    })
    .body(|ui| {
        for schema in &mut db.schemas {
            render_schema_node(ui, schema);
        }
    });
}

fn render_schema_node(ui: &mut egui::Ui, schema: &mut ExportSchemaItem) {
    let total = schema.tables.len();
    let selected = schema.tables.iter().filter(|t| t.selected).count();

    let id = ui.make_persistent_id(format!("export_schema_{}_{}", schema.database, schema.name));
    let cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        schema.expanded,
    );

    cs.show_header(ui, |ui| {
        let tri = tristate(selected, total);
        let mut new_checked = tri == TriState::Checked;
        if tristate_checkbox(ui, &mut new_checked, tri).changed() {
            let target = tri == TriState::Unchecked;
            for t in &mut schema.tables {
                t.selected = target;
            }
        }
        ui.label(crate::ui::icons::db::schema(
            14.0,
            crate::ui::icons::db::COLOR_SCHEMA,
        ));
        ui.label(&schema.name);
    })
    .body(|ui| {
        for tbl in &mut schema.tables {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.checkbox(&mut tbl.selected, "");
                if tbl.is_view {
                    ui.label(crate::ui::icons::db::view(
                        14.0,
                        crate::ui::icons::db::COLOR_VIEW,
                    ));
                } else {
                    ui.label(crate::ui::icons::db::table(
                        14.0,
                        crate::ui::icons::db::COLOR_TABLE,
                    ));
                }
                ui.label(&tbl.name);
            });
        }
    });
}

// ── Tristate checkbox ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriState {
    Unchecked,
    Mixed,
    Checked,
}

fn tristate(selected: usize, total: usize) -> TriState {
    if selected == 0 {
        TriState::Unchecked
    } else if selected == total {
        TriState::Checked
    } else {
        TriState::Mixed
    }
}

/// Tristate checkbox — painter-based widget with unchecked / mixed / checked states.
fn tristate_checkbox(ui: &mut egui::Ui, checked: &mut bool, tri: TriState) -> egui::Response {
    let size = egui::vec2(16.0, 16.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }

    let visuals = ui.style().interact(&resp);
    let painter = ui.painter();
    let rounding = 2.0;
    let box_rect = rect.shrink(1.5);

    match tri {
        TriState::Unchecked => {
            painter.rect_stroke(
                box_rect,
                rounding,
                egui::Stroke::new(1.0, visuals.bg_stroke.color),
                egui::StrokeKind::Inside,
            );
        }
        TriState::Checked => {
            painter.rect_filled(box_rect, rounding, egui::Color32::from_rgb(59, 130, 246));
            let c = box_rect.center();
            painter.line_segment(
                [egui::pos2(c.x - 3.5, c.y), egui::pos2(c.x - 1.0, c.y + 2.5)],
                egui::Stroke::new(1.8, egui::Color32::WHITE),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - 1.0, c.y + 2.5),
                    egui::pos2(c.x + 3.5, c.y - 2.0),
                ],
                egui::Stroke::new(1.8, egui::Color32::WHITE),
            );
        }
        TriState::Mixed => {
            painter.rect_filled(box_rect, rounding, egui::Color32::from_rgb(59, 130, 246));
            let c = box_rect.center();
            painter.line_segment(
                [egui::pos2(c.x - 4.0, c.y), egui::pos2(c.x + 4.0, c.y)],
                egui::Stroke::new(2.0, egui::Color32::WHITE),
            );
        }
    }

    resp
}
