/// Drains DbWorker events and native macOS menu actions each frame.
/// Extracted from `app.rs` to keep the main application file focused on
/// struct definition, construction, and the eframe trait impl glue.
use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::types::{DatabaseNode, SchemaTree};
use suprim_core::storage::QueryHistoryEntry;

use crate::app::App;

impl App {
    /// Drain all pending events from the DbWorker and update state.
    /// Returns `true` if at least one event was processed.
    pub(crate) fn process_events(&mut self) -> bool {
        use suprim_core::db::commands::DbEvent;

        let mut had_events = false;
        while let Ok(event) = self.event_rx.try_recv() {
            had_events = true;
            match event {
                DbEvent::Connected {
                    conn_id,
                    databases,
                    server_version,
                } => {
                    // Forward to structure sync dialog if open
                    if let Some(dialog) = &mut self.structure_sync_dialog {
                        dialog.update_databases(conn_id, databases.clone());
                        dialog.update_server_version(conn_id, server_version.clone());
                    }
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
                    self.sidebar.on_connected(
                        conn_id,
                        conn_name,
                        schema,
                        visible_dbs,
                        server_version,
                    );
                    self.status = "Connected".to_string();
                }
                DbEvent::Disconnected { conn_id } => {
                    self.sidebar.on_disconnected(conn_id);
                    self.status = "Disconnected".to_string();
                }
                DbEvent::QueryResult { tab_id, result } => {
                    let row_count = result.rows.len();
                    let millis = result.execution_time.as_millis();
                    // Record to query history
                    if let Some((sql, conn_name, database)) =
                        self.tab_manager.get_tab_query_info(tab_id)
                    {
                        self.history.add(QueryHistoryEntry {
                            sql,
                            conn_name,
                            database,
                            timestamp: chrono::Utc::now(),
                            execution_time_ms: millis as u64,
                            row_count,
                            rows_affected: result.rows_affected,
                            success: true,
                            error_message: None,
                        });
                    }
                    self.tab_manager.on_query_result(tab_id, result);
                    self.status =
                        format!("Query complete \u{2014} {row_count} rows  ({millis} ms)");
                }
                DbEvent::DatabasesListed { conn_id, databases } => {
                    // Forward to structure sync dialog if open
                    if let Some(dialog) = &mut self.structure_sync_dialog {
                        dialog.update_databases(conn_id, databases.clone());
                    }
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
                    self.sidebar
                        .on_schemas_listed(conn_id, &database, schemas.clone());
                    // Forward to structure sync dialog if open
                    if let Some(dialog) = &mut self.structure_sync_dialog {
                        dialog.update_schemas(conn_id, &database, schemas);
                    }
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
                DbEvent::DatabaseCreated { conn_id } => {
                    let _ = self.cmd_tx.try_send(DbCommand::ListDatabases { conn_id });
                    self.status = "Database created".to_string();
                }
                DbEvent::SchemaCreated { conn_id, database } => {
                    let _ = self
                        .cmd_tx
                        .try_send(DbCommand::ListSchemas { conn_id, database });
                    self.status = "Schema created".to_string();
                }
                DbEvent::Error {
                    tab_id,
                    conn_id,
                    message,
                } => {
                    if let Some(tid) = tab_id {
                        // Record failed query to history
                        if let Some((sql, conn_name, database)) =
                            self.tab_manager.get_tab_query_info(tid)
                        {
                            self.history.add(QueryHistoryEntry {
                                sql,
                                conn_name,
                                database,
                                timestamp: chrono::Utc::now(),
                                execution_time_ms: 0,
                                row_count: 0,
                                rows_affected: 0,
                                success: false,
                                error_message: Some(message.clone()),
                            });
                        }
                        self.tab_manager.on_tab_error(tid);
                    }
                    // If error has conn_id but no tab_id, it's a connection-level error
                    // (e.g. connect failed). Mark the sidebar entry as failed.
                    if tab_id.is_none() {
                        if let Some(cid) = conn_id {
                            self.sidebar.on_connect_failed(cid, message.clone());
                        }
                    }
                    self.status = format!("Error: {message}");
                }
                DbEvent::TestConnectionResult { success, message } => {
                    if let Some(dialog) = &mut self.connection_dialog {
                        dialog.on_test_result(success, message);
                    }
                }
                DbEvent::SchemasCompared {
                    source,
                    target,
                    source_extensions,
                    target_extensions,
                } => {
                    if let Some(dialog) = &mut self.structure_sync_dialog {
                        dialog.on_schemas_compared(
                            source,
                            target,
                            source_extensions,
                            target_extensions,
                        );
                    }
                }
                DbEvent::DashboardLoaded {
                    conn_id,
                    sessions,
                    metrics,
                    slow_queries,
                } => {
                    self.tab_manager
                        .on_dashboard_loaded(conn_id, sessions, metrics, slow_queries);
                }
                DbEvent::SessionKilled { conn_id, pid } => {
                    self.status = format!("Session PID {} terminated", pid);
                    // Trigger a dashboard refresh
                    let _ = self.cmd_tx.try_send(DbCommand::LoadDashboard { conn_id });
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
                    if let Err(msg) = self.gate.can_add_connection(self.config.connections.len()) {
                        self.upgrade_prompt = Some(crate::ui::UpgradePrompt::new(&msg));
                    } else {
                        self.connection_dialog = Some(crate::ui::ConnectionDialog::new());
                    }
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
                MenuAction::QueryHistory => {
                    self.show_history = !self.show_history;
                }
                MenuAction::License => {
                    self.open_license_dialog();
                }
                MenuAction::DataTransfer
                | MenuAction::DataGeneration
                | MenuAction::DataDictionary
                | MenuAction::DataSynchronization => {
                    // TODO: implement Tools dialogs
                    self.status = format!("{action:?} — coming soon");
                }
                MenuAction::StructureSynchronization => {
                    use suprim_core::db::connection::DriverParams;
                    use suprim_core::sync_types::{ConnInfo, ConnMeta, DbInfo};

                    // Build ConnInfo from active (connected) connections only
                    let conns: Vec<ConnInfo> = self
                        .sidebar
                        .connection_list()
                        .into_iter()
                        .map(|(conn_id, label, dbs, server_version, connected)| {
                            let (host, port, driver_type) = self
                                .config
                                .connections
                                .iter()
                                .find(|c| c.id == conn_id)
                                .map(|cfg| {
                                    let (h, p) = match &cfg.params {
                                        DriverParams::Postgres { host, port, .. }
                                        | DriverParams::Mysql { host, port, .. }
                                        | DriverParams::Mssql { host, port, .. } => {
                                            (host.clone(), port.to_string())
                                        }
                                        DriverParams::Redis { host, port, .. } => {
                                            (host.clone(), port.to_string())
                                        }
                                        DriverParams::Sqlite { path } => {
                                            (path.display().to_string(), String::new())
                                        }
                                        DriverParams::MongoDB { uri, .. } => {
                                            (uri.clone(), String::new())
                                        }
                                    };
                                    (h, p, cfg.driver_type().to_string())
                                })
                                .unwrap_or_default();

                            let databases = dbs
                                .into_iter()
                                .map(|(name, schemas)| DbInfo { name, schemas })
                                .collect();

                            ConnInfo {
                                conn_id,
                                label,
                                databases,
                                meta: ConnMeta {
                                    driver_type,
                                    host,
                                    port,
                                    server_version,
                                },
                                connected,
                            }
                        })
                        .collect();

                    if let Some(dialog) = self.gate.create_structure_sync(conns) {
                        self.structure_sync_dialog = Some(dialog);
                    } else {
                        // Premium feature not available — show upgrade prompt
                        self.upgrade_prompt = Some(crate::ui::UpgradePrompt::new(
                            "Structure Synchronization is a Premium feature.",
                        ));
                    }
                }
            }
            ctx.request_repaint();
        }
    }
}
