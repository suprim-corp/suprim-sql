use std::collections::HashSet;

use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::SchemaTree;
use uuid::Uuid;

use super::sequences_folder_renderer;
use super::tables_folder_renderer;
use super::views_folder_renderer;
use super::SidebarAction;
use crate::ui::icons;

/// Render the full schema tree for one connection.
/// Returns an optional action (open viewer, lazy-load trigger, etc.).
pub(super) fn render_schema_tree(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    schema: &SchemaTree,
    visible_databases: Option<&Vec<String>>,
    schema_detail_requested: &mut HashSet<String>,
    schemas_requested: &mut HashSet<String>,
) -> Option<SidebarAction> {
    let mut action: Option<SidebarAction> = None;

    for db_node in &schema.databases {
        if let Some(names) = &visible_databases {
            if !names.contains(&db_node.name) {
                continue;
            }
        }

        let db_name = db_node.name.clone();

        let db_id_salt = format!("{conn_id}:{}", db_node.name);
        let db_state_id = ui.make_persistent_id(&db_id_salt);
        let db_state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            db_state_id,
            false,
        );
        let openness = db_state.openness(ui.ctx());

        let (toggle_resp, header_resp, _body_resp) = db_state
            .show_header(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(icons::db::database(
                        icons::SIDEBAR_ICON,
                        icons::db::COLOR_DATABASE,
                    ));
                    ui.selectable_label(false, &db_node.name)
                })
            })
            .body(|ui| {
                for schema_node in &db_node.schemas {
                    render_schema_node(
                        ui,
                        conn_id,
                        &db_name,
                        schema_node,
                        schema_detail_requested,
                        &mut action,
                    );
                }
                if db_node.schemas.is_empty() {
                    ui.weak("loading schemas...");
                }
            });
        // Click label → toggle expand/collapse. Must run before
        // `on_hover_cursor` (which consumes the response).
        super::sidebar_renderer::toggle_on_label_click(
            ui.ctx(),
            db_state_id,
            &header_resp.inner.inner,
        );

        let db_header = header_resp
            .inner
            .inner
            .on_hover_cursor(CursorIcon::PointingHand);
        toggle_resp.on_hover_cursor(CursorIcon::PointingHand);

        // Context menu on database node
        db_header.context_menu(|ui| {
            if ui
                .button("New Schema...")
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                action = Some(SidebarAction::NewSchema {
                    conn_id,
                    database: db_node.name.clone(),
                });
                ui.close();
            }
        });

        // Trigger ListSchemas when database expanded but has no schemas yet.
        if openness > 0.0
            && db_node.schemas.is_empty()
            && action.is_none()
            && !schemas_requested.contains(&db_name)
        {
            schemas_requested.insert(db_name.clone());
            action = Some(SidebarAction::ListSchemas {
                conn_id,
                database: db_name,
            });
        }
    }

    action
}

/// Render a single schema node (e.g. "public") with its folders.
fn render_schema_node(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_node: &suprim_core::db::types::SchemaNode,
    schema_detail_requested: &mut HashSet<String>,
    action: &mut Option<SidebarAction>,
) {
    let schema_name = &schema_node.name;
    let loaded = schema_node.loaded;

    let suffix = if loaded { "" } else { " ..." };

    let schema_id = egui::Id::new(format!("{conn_id}:{db_name}:{schema_name}"));
    let schema_state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        schema_id,
        false,
    );
    let openness = schema_state.openness(ui.ctx());

    let (toggle_resp, header_resp, _body_resp) = schema_state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::schema(
                    icons::SIDEBAR_ICON,
                    icons::db::COLOR_SCHEMA,
                ));
                ui.selectable_label(false, format!("{}{}", schema_name, suffix))
            })
        })
        .body(|ui| {
            if !loaded {
                ui.weak("loading...");
                return;
            }

            // Always show all object folders (with count, even if 0).
            tables_folder_renderer::render_tables_folder(
                ui,
                conn_id,
                db_name,
                schema_name,
                schema_node,
                action,
            );
            if !schema_node.views.is_empty() {
                views_folder_renderer::render_views_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    &schema_node.views,
                    action,
                );
            }
            if !schema_node.materialized_views.is_empty() {
                views_folder_renderer::render_materialized_views_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    &schema_node.materialized_views,
                    action,
                );
            }
            sequences_folder_renderer::render_sequences_folder(
                ui,
                conn_id,
                db_name,
                schema_name,
                schema_node,
                action,
            );
        });
    // Click schema label → toggle expand/collapse. Must run before
    // `on_hover_cursor` (which consumes the response).
    super::sidebar_renderer::toggle_on_label_click(
        ui.ctx(),
        schema_id,
        &header_resp.inner.inner,
    );

    // Right-click context menu on schema label.
    // Note: Drop Schema is intentionally omitted here — that requires a new
    // DbCommand + dialect impl + confirmation dialog. Track in follow-up.
    header_resp.inner.inner.context_menu(|ui| {
        if ui
            .button(format!("{}  Refresh", icons::ph::arrows_clockwise()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::RefreshSchema {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
            });
            ui.close();
        }
        if ui
            .button(format!("{}  New Table...", icons::ph::plus_circle()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::NewTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                schema_functions: schema_node
                    .functions
                    .iter()
                    .map(|f| f.signature.clone())
                    .collect(),
            });
            ui.close();
        }
    });

    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .inner
        .on_hover_cursor(CursorIcon::PointingHand);

    // Trigger lazy-load when expanded but not yet loaded.
    let detail_key = format!("{db_name}:{schema_name}");
    if openness > 0.0
        && !loaded
        && action.is_none()
        && !schema_detail_requested.contains(&detail_key)
    {
        schema_detail_requested.insert(detail_key);
        *action = Some(SidebarAction::LoadSchemaDetail {
            conn_id,
            database: db_name.to_owned(),
            schema_name: schema_name.to_owned(),
        });
    }
}
