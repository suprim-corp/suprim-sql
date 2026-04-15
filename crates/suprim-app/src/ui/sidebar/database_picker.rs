use eframe::egui;
use suprim_sql::db::types::DatabaseNode;

/// Render the database filter/picker popup window.
/// Returns `Some(new_visible)` if the user changed the filter, `None` otherwise.
/// Sets `picker_open` to `false` when the user clicks Close.
pub(super) fn render_database_picker(
    ctx: &egui::Context,
    label: &str,
    picker_id: egui::Id,
    picker_open: &mut bool,
    all_databases: &[DatabaseNode],
    visible_databases: &Option<Vec<String>>,
) -> Option<Option<Vec<String>>> {
    let mut new_visible: Option<Option<Vec<String>>> = None;
    let mut close_picker = false;

    egui::Window::new(format!("Filter databases - {}", label))
        .id(picker_id)
        .collapsible(false)
        .resizable(false)
        .min_width(260.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Select databases to show:");
            ui.add_space(4.0);

            let all_selected = visible_databases.is_none();
            let mut show_all = all_selected;
            if ui.checkbox(&mut show_all, "Show all").changed() {
                new_visible = Some(if show_all { None } else { Some(vec![]) });
            }

            ui.separator();

            let current_visible: Vec<String> = if all_selected {
                all_databases.iter().map(|d| d.name.clone()).collect()
            } else {
                visible_databases.clone().unwrap_or_default()
            };

            for db in all_databases {
                let mut checked = all_selected || current_visible.contains(&db.name);
                let prev = checked;
                ui.checkbox(&mut checked, &db.name);
                if checked != prev {
                    let mut updated: Vec<String> = current_visible.clone();
                    if checked {
                        if !updated.contains(&db.name) {
                            updated.push(db.name.clone());
                        }
                    } else {
                        updated.retain(|n| n != &db.name);
                    }
                    new_visible = Some(if updated.len() == all_databases.len() {
                        None
                    } else {
                        Some(updated)
                    });
                }
            }

            ui.add_space(6.0);
            if ui
                .button("Close")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                close_picker = true;
            }
        });

    if close_picker {
        *picker_open = false;
    }

    new_visible
}
