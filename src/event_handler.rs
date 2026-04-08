/// Drains DbWorker events and native macOS menu actions each frame.
/// Extracted from `app.rs` to keep the main application file focused on
/// struct definition, construction, and the eframe trait impl glue.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::{DatabaseNode, SchemaTree};

use crate::app::App;

impl App {
    /// Drain all pending events from the DbWorker and update state.
    /// Returns `true` if at least one event was processed.
    pub(crate) fn process_events(&mut self) -> bool {
        use suprim_sql::db::driver::DbEvent;

        let mut had_events = false;
        while let Ok(event) = self.event_rx.try_recv() {
            had_events = true;
            match event {
                DbEvent::Connected { conn_id, databases } => {
                    let schema = SchemaTree {
                        databases: databases
                            .into_iter()
                            .map(|name| DatabaseNode {
                                id: uuid::Uuid::new_v4(),
                                name,
                                schemas: vec![],
                            })
                            .collect(),
                    };
                    let saved = self.config.connections.iter().find(|c| c.id == conn_id);
                    let conn_name = saved
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| conn_id.to_string());
                    let visible_dbs = saved.and_then(|c| c.visible_databases.clone());
                    self.sidebar
                        .on_connected(conn_id, conn_name, schema, visible_dbs);
                    self.status = "Connected".to_string();
                }
                DbEvent::Disconnected { conn_id } => {
                    self.sidebar.on_disconnected(conn_id);
                    self.config.remove_connection(conn_id);
                    self.status = "Disconnected".to_string();
                }
                DbEvent::QueryResult { tab_id, result } => {
                    let row_count = result.rows.len();
                    let millis = result.execution_time.as_millis();
                    self.tab_manager.on_query_result(tab_id, result);
                    self.status =
                        format!("Query complete \u{2014} {row_count} rows  ({millis} ms)");
                }
                DbEvent::DatabasesListed { conn_id, databases } => {
                    let schema = SchemaTree {
                        databases: databases
                            .into_iter()
                            .map(|name| DatabaseNode {
                                id: uuid::Uuid::new_v4(),
                                name,
                                schemas: vec![],
                            })
                            .collect(),
                    };
                    self.sidebar.on_schema_loaded(conn_id, schema);
                }
                DbEvent::SchemasListed {
                    conn_id,
                    database,
                    schemas,
                } => {
                    self.sidebar.on_schemas_listed(conn_id, &database, schemas);
                }
                DbEvent::SchemaDetailLoaded {
                    conn_id,
                    database,
                    schema_name,
                    schema_node,
                } => {
                    self.sidebar.on_schema_detail_loaded(
                        conn_id,
                        &database,
                        &schema_name,
                        schema_node,
                    );
                }
                DbEvent::RowMutated {
                    tab_id,
                    rows_affected,
                } => {
                    self.tab_manager.on_row_mutated(tab_id, rows_affected);
                    self.status = format!("{rows_affected} row(s) affected");
                }
                DbEvent::DdlCompleted {
                    conn_id,
                    database,
                    schema_name,
                } => {
                    let _ = self.cmd_tx.try_send(DbCommand::LoadSchemaDetail {
                        conn_id,
                        database,
                        schema_name,
                    });
                    self.status = "Operation completed".to_string();
                }
                DbEvent::Error {
                    tab_id, message, ..
                } => {
                    if let Some(tid) = tab_id {
                        self.tab_manager.on_tab_error(tid);
                    }
                    self.status = format!("Error: {message}");
                }
            }
        }
        had_events
    }

    /// Process native macOS menu actions each frame.
    #[cfg(target_os = "macos")]
    pub(crate) fn process_menu_actions(&mut self, ctx: &egui::Context) {
        use crate::ui::macos_menu::MenuAction;

        while let Ok(action) = self.native_menu.rx.try_recv() {
            match action {
                MenuAction::NewConnection => {
                    self.connection_dialog = Some(crate::ui::ConnectionDialog::new());
                }
                MenuAction::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                MenuAction::NewSqlTab => {
                    if let Some((conn_id, name, database, databases)) =
                        self.sidebar.first_connection_info()
                    {
                        self.tab_manager
                            .open_sql_tab(Some(conn_id), name, database, databases);
                    } else {
                        self.tab_manager
                            .open_sql_tab(None, String::new(), None, Vec::new());
                    }
                }
                MenuAction::ReloadDatabases => {
                    for conn_id in self.sidebar.active_connection_ids() {
                        let _ = self.cmd_tx.try_send(DbCommand::ListDatabases { conn_id });
                    }
                }
            }
            ctx.request_repaint();
        }
    }
}
