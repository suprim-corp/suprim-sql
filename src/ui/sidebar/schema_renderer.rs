use std::collections::HashSet;

use eframe::egui;
use suprim_sql::db::types::SchemaTree;
use uuid::Uuid;

use super::folder_renderers;
use super::SidebarAction;

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
        let db_label = format!("{} {}", egui_phosphor::regular::DATABASE, db_node.name);

        let db_response = egui::CollapsingHeader::new(&db_label)
            .id_salt(format!("{conn_id}:{}", db_node.name))
            .show(ui, |ui| {
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

        // Trigger ListSchemas when database expanded but has no schemas yet.
        if db_response.openness > 0.0
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
    schema_node: &suprim_sql::db::types::SchemaNode,
    schema_detail_requested: &mut HashSet<String>,
    action: &mut Option<SidebarAction>,
) {
    let schema_name = &schema_node.name;
    let loaded = schema_node.loaded;

    let display = if loaded {
        format!("{} {}", egui_phosphor::regular::TREE_STRUCTURE, schema_name)
    } else {
        format!(
            "{} {} ...",
            egui_phosphor::regular::TREE_STRUCTURE,
            schema_name
        )
    };

    let schema_id = egui::Id::new(format!("{conn_id}:{db_name}:{schema_name}"));
    let resp = egui::CollapsingHeader::new(display)
        .id_salt(schema_id)
        .show(ui, |ui| {
            if !loaded {
                ui.weak("loading...");
                return;
            }

            let has_tables = !schema_node.tables.is_empty();
            let has_views = !schema_node.views.is_empty();
            let has_matviews = !schema_node.materialized_views.is_empty();
            let has_sequences = !schema_node.sequences.is_empty();

            if !has_tables && !has_views && !has_matviews && !has_sequences {
                ui.weak("(empty)");
                return;
            }

            if has_tables {
                folder_renderers::render_tables_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    schema_node,
                    action,
                );
            }
            if has_views {
                folder_renderers::render_views_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    &schema_node.views,
                    action,
                );
            }
            if has_matviews {
                folder_renderers::render_materialized_views_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    &schema_node.materialized_views,
                    action,
                );
            }
            if has_sequences {
                folder_renderers::render_sequences_folder(
                    ui,
                    conn_id,
                    db_name,
                    schema_name,
                    schema_node,
                );
            }
        });

    // Trigger lazy-load when expanded but not yet loaded.
    let detail_key = format!("{db_name}:{schema_name}");
    if resp.openness > 0.0
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
