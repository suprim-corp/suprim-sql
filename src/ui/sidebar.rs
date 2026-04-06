use eframe::egui;
use suprim_sql::db::types::{DatabaseNode, SchemaTree};
use uuid::Uuid;

/// Action the sidebar wants the app to perform.
pub enum SidebarAction {
    NewConnection,
    EditConnection {
        conn_id: Uuid,
    },
    OpenSqlTab {
        conn_id: Uuid,
    },
    OpenTableViewer {
        conn_id: Uuid,
        table_name: String,
    },
    Disconnect {
        conn_id: Uuid,
    },
    /// Request lazy-load of a schema's tables/views.
    LoadSchemaDetail {
        conn_id: Uuid,
        schema_name: String,
    },
    /// User updated the visible databases filter for a connection.
    UpdateVisibleDatabases {
        conn_id: Uuid,
        /// None = show all
        visible: Option<Vec<String>>,
    },
}

/// A single connection entry shown in the sidebar.
struct ConnectionEntry {
    conn_id: Uuid,
    label: String,
    /// Currently displayed schema tree (already filtered).
    schema: Option<SchemaTree>,
    /// ALL databases returned from the server (unfiltered) — for the picker.
    all_databases: Vec<DatabaseNode>,
    /// Which database names are visible. None = all.
    visible_databases: Option<Vec<String>>,
    expanded: bool,
    /// Whether the db-picker popup is open.
    picker_open: bool,
    /// Schema names that have already had a LoadSchemaDetail request sent.
    schema_detail_requested: std::collections::HashSet<String>,
}

/// The left-hand schema / connection browser panel.
pub struct Sidebar {
    connections: Vec<ConnectionEntry>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    pub fn active_connection_ids(&self) -> Vec<Uuid> {
        self.connections.iter().map(|c| c.conn_id).collect()
    }

    pub fn on_connected(&mut self, conn_id: Uuid, name: String, schema: SchemaTree) {
        self.connections.retain(|c| c.conn_id != conn_id);
        let all_databases = schema.databases.clone();
        self.connections.push(ConnectionEntry {
            conn_id,
            label: name,
            schema: Some(schema),
            all_databases,
            visible_databases: None,
            expanded: true,
            picker_open: false,
            schema_detail_requested: std::collections::HashSet::new(),
        });
    }

    pub fn on_disconnected(&mut self, conn_id: Uuid) {
        self.connections.retain(|c| c.conn_id != conn_id);
    }

    pub fn on_schema_loaded(&mut self, conn_id: Uuid, schema: SchemaTree) {
        if let Some(entry) = self.connections.iter_mut().find(|c| c.conn_id == conn_id) {
            entry.all_databases = schema.databases.clone();
            entry.schema = Some(schema);
            entry.schema_detail_requested.clear();
        }
    }

    pub fn on_schema_detail_loaded(
        &mut self,
        conn_id: Uuid,
        schema_name: &str,
        schema_node: suprim_sql::db::types::SchemaNode,
    ) {
        if let Some(entry) = self.connections.iter_mut().find(|c| c.conn_id == conn_id) {
            if let Some(tree) = &mut entry.schema {
                for db in &mut tree.databases {
                    for schema in &mut db.schemas {
                        if schema.name == schema_name {
                            *schema = schema_node;
                            entry.schema_detail_requested.remove(schema_name);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Render the sidebar. Returns an optional action to execute.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<SidebarAction> {
        let mut action: Option<SidebarAction> = None;

        ui.heading("Connections");
        ui.separator();
        ui.add_space(4.0);

        let mut disconnect_id: Option<Uuid> = None;

        for entry in &mut self.connections {
            let conn_id = entry.conn_id;
            let truncated_label = truncate_label(&entry.label, 24);

            // Header with db count badge
            let total = entry.all_databases.len();
            let shown = match &entry.visible_databases {
                None => total,
                Some(v) => v.len(),
            };
            let is_filtered = entry.visible_databases.is_some();
            let badge = if is_filtered {
                format!("{}/{}", shown, total)
            } else {
                total.to_string()
            };
            let header_label = format!("{}  [{}]", truncated_label, badge);

            let header = egui::CollapsingHeader::new(&header_label)
                .default_open(entry.expanded)
                .id_salt(conn_id);

            let response = header.show(ui, |ui| {
                // Filter databases by visible list.
                // None = show all, Some(vec) = only show those in vec (even if empty).
                let visible_names: Option<&Vec<String>> = entry.visible_databases.as_ref();

                if let Some(schema) = &entry.schema {
                    for db_node in &schema.databases {
                        // Skip databases not in the visible filter.
                        if let Some(names) = &visible_names {
                            if !names.contains(&db_node.name) {
                                continue;
                            }
                        }
                        egui::CollapsingHeader::new(&db_node.name)
                            .id_salt(format!("{conn_id}:{}", db_node.name))
                            .show(ui, |ui| {
                                for schema_node in &db_node.schemas {
                                    let schema_name = schema_node.name.clone();
                                    let loaded = schema_node.loaded;

                                    let schema_id = egui::Id::new(format!(
                                        "{conn_id}:{}:{}",
                                        db_node.name, schema_node.name
                                    ));

                                    let display_name = if loaded {
                                        schema_name.clone()
                                    } else {
                                        format!("{} ...", schema_name)
                                    };

                                    let schema_response = egui::CollapsingHeader::new(display_name)
                                        .id_salt(schema_id)
                                        .show(ui, |ui| {
                                            for table_node in &schema_node.tables {
                                                let tbl = table_node.name.clone();
                                                let btn = egui::Button::new(&tbl).frame(false);
                                                if ui.add(btn).double_clicked() {
                                                    action = Some(SidebarAction::OpenTableViewer {
                                                        conn_id,
                                                        table_name: tbl,
                                                    });
                                                }
                                            }
                                            for view_node in &schema_node.views {
                                                let v = view_node.name.clone();
                                                let btn = egui::Button::new(&v).frame(false);
                                                if ui.add(btn).double_clicked() {
                                                    action = Some(SidebarAction::OpenTableViewer {
                                                        conn_id,
                                                        table_name: v,
                                                    });
                                                }
                                            }
                                            if schema_node.tables.is_empty()
                                                && schema_node.views.is_empty()
                                            {
                                                if loaded {
                                                    ui.weak("(empty)");
                                                } else {
                                                    ui.weak("loading...");
                                                }
                                            }
                                        });

                                    if schema_response.openness > 0.0 && !loaded && action.is_none()
                                    {
                                        if !entry.schema_detail_requested.contains(&schema_name) {
                                            entry
                                                .schema_detail_requested
                                                .insert(schema_name.clone());
                                            action = Some(SidebarAction::LoadSchemaDetail {
                                                conn_id,
                                                schema_name: schema_name.clone(),
                                            });
                                        }
                                    }
                                }
                                if db_node.schemas.is_empty() {
                                    ui.weak("(no schemas)");
                                }
                            });
                    }
                }
            });

            // Right-click context menu
            response.header_response.context_menu(|ui| {
                if ui.button("New SQL Tab").clicked() {
                    action = Some(SidebarAction::OpenSqlTab { conn_id });
                    ui.close();
                }
                ui.separator();
                if ui.button("Filter Databases...").clicked() {
                    entry.picker_open = !entry.picker_open;
                    ui.close();
                }
                if ui.button("Edit Connection...").clicked() {
                    action = Some(SidebarAction::EditConnection { conn_id });
                    ui.close();
                }
                if ui.button("Disconnect").clicked() {
                    disconnect_id = Some(conn_id);
                    ui.close();
                }
            });

            // DB-picker popup
            if entry.picker_open {
                let picker_id = egui::Id::new(format!("db_picker_{conn_id}"));
                let mut close_picker = false;
                let mut new_visible: Option<Option<Vec<String>>> = None;

                egui::Window::new(format!("Filter databases - {}", truncated_label))
                    .id(picker_id)
                    .collapsible(false)
                    .resizable(false)
                    .min_width(260.0)
                    .show(ui.ctx(), |ui| {
                        ui.label("Select databases to show:");
                        ui.add_space(4.0);

                        // None = show all, Some(_) = filtered (even if empty = hide all)
                        let all_selected = entry.visible_databases.is_none();
                        let mut show_all = all_selected;
                        if ui.checkbox(&mut show_all, "Show all").changed() {
                            if show_all {
                                new_visible = Some(None);
                            } else {
                                // Uncheck "show all" → start with empty (hide all)
                                new_visible = Some(Some(vec![]));
                            }
                        }

                        ui.separator();

                        let current_visible: Vec<String> = if all_selected {
                            // "Show all" means all names are implicitly selected.
                            entry.all_databases.iter().map(|d| d.name.clone()).collect()
                        } else {
                            entry.visible_databases.clone().unwrap_or_default()
                        };

                        for db in &entry.all_databases {
                            let mut checked = all_selected || current_visible.contains(&db.name);
                            let prev = checked;
                            ui.checkbox(&mut checked, &db.name);
                            if checked != prev {
                                let mut updated: Vec<String> =
                                    current_visible.iter().cloned().collect();
                                if checked {
                                    if !updated.contains(&db.name) {
                                        updated.push(db.name.clone());
                                    }
                                } else {
                                    updated.retain(|n| n != &db.name);
                                }
                                if updated.len() == entry.all_databases.len() {
                                    new_visible = Some(None);
                                } else {
                                    new_visible = Some(Some(updated));
                                }
                            }
                        }

                        ui.add_space(6.0);
                        if ui.button("Close").clicked() {
                            close_picker = true;
                        }
                    });

                if close_picker {
                    entry.picker_open = false;
                }
                if let Some(visible) = new_visible {
                    entry.visible_databases = visible.clone();
                    if action.is_none() {
                        action = Some(SidebarAction::UpdateVisibleDatabases { conn_id, visible });
                    }
                }
            }
        }

        if let Some(id) = disconnect_id {
            action = Some(SidebarAction::Disconnect { conn_id: id });
        }

        action
    }
}

fn truncate_label(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{}...", truncated)
    }
}
