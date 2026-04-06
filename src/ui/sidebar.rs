use eframe::egui;
use suprim_sql::db::types::SchemaTree;
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
}

/// A single connection entry shown in the sidebar.
struct ConnectionEntry {
    conn_id: Uuid,
    label: String,
    schema: Option<SchemaTree>,
    expanded: bool,
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
        // Remove duplicate if reconnecting.
        self.connections.retain(|c| c.conn_id != conn_id);
        self.connections.push(ConnectionEntry {
            conn_id,
            label: name,
            schema: Some(schema),
            expanded: true,
        });
    }

    pub fn on_disconnected(&mut self, conn_id: Uuid) {
        self.connections.retain(|c| c.conn_id != conn_id);
    }

    pub fn on_schema_loaded(&mut self, conn_id: Uuid, schema: SchemaTree) {
        if let Some(entry) = self.connections.iter_mut().find(|c| c.conn_id == conn_id) {
            entry.schema = Some(schema);
        }
    }

    /// Patch in a freshly-loaded SchemaNode for a given conn_id + schema_name.
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
            let label = entry.label.clone();

            let header = egui::CollapsingHeader::new(&label)
                .default_open(entry.expanded)
                .id_salt(conn_id);

            let response = header.show(ui, |ui| {
                // Schema tree
                if let Some(schema) = &entry.schema {
                    for db_node in &schema.databases {
                        egui::CollapsingHeader::new(&db_node.name)
                            .id_salt(format!("{conn_id}:{}", db_node.name))
                            .show(ui, |ui| {
                                for schema_node in &db_node.schemas {
                                    let schema_name = schema_node.name.clone();
                                    let loaded = schema_node.loaded;

                                    // Detect expansion: if this header is open and not yet
                                    // loaded, request lazy load.
                                    let schema_id = egui::Id::new(format!(
                                        "{conn_id}:{}:{}",
                                        db_node.name, schema_node.name
                                    ));

                                    // Show loading indicator in label when not loaded.
                                    let display_name = if !loaded {
                                        format!("{} ⏳", schema_name)
                                    } else {
                                        schema_name.clone()
                                    };

                                    let schema_response = egui::CollapsingHeader::new(display_name)
                                        .id_salt(schema_id)
                                        .show(ui, |ui| {
                                            for table_node in &schema_node.tables {
                                                let tbl = table_node.name.clone();
                                                let btn = egui::Button::new(format!("🗂 {tbl}"))
                                                    .frame(false);
                                                if ui.add(btn).double_clicked() {
                                                    action = Some(SidebarAction::OpenTableViewer {
                                                        conn_id,
                                                        table_name: tbl,
                                                    });
                                                }
                                            }
                                            for view_node in &schema_node.views {
                                                let v = view_node.name.clone();
                                                let btn = egui::Button::new(format!("👁 {v}"))
                                                    .frame(false);
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
                                                    ui.weak("Loading…");
                                                }
                                            }
                                        });

                                    // If header just became open and schema is not loaded,
                                    // request load.
                                    if schema_response.openness > 0.0 && !loaded && action.is_none()
                                    {
                                        action = Some(SidebarAction::LoadSchemaDetail {
                                            conn_id,
                                            schema_name: schema_name.clone(),
                                        });
                                    }
                                }
                                if db_node.schemas.is_empty() {
                                    ui.weak("(no schemas)");
                                }
                            });
                    }
                }
            });

            // Right-click context menu on the header.
            response.header_response.context_menu(|ui| {
                if ui.button("New SQL Tab").clicked() {
                    action = Some(SidebarAction::OpenSqlTab { conn_id });
                    ui.close();
                }
                ui.separator();
                if ui.button("Edit Connection…").clicked() {
                    action = Some(SidebarAction::EditConnection { conn_id });
                    ui.close();
                }
                if ui.button("Disconnect").clicked() {
                    disconnect_id = Some(conn_id);
                    ui.close();
                }
            });
        }

        if let Some(id) = disconnect_id {
            action = Some(SidebarAction::Disconnect { conn_id: id });
        }

        action
    }
}
