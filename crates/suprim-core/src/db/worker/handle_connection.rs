/// Connection lifecycle handlers: connect, test connect, and disconnect.
use uuid::Uuid;

use crate::db::commands::DbEvent;
use crate::db::factory::DbFactory;
use crate::db::ssh_tunnel::SshTunnel;

use super::DbWorker;

impl DbWorker {
    pub(super) async fn handle_connect(
        &mut self,
        config: crate::db::connection::ConnectionConfig,
    ) {
        let conn_id = config.id;

        // Establish SSH tunnel if configured
        let mut tunnel_config = config.clone();
        let tunnel = if let Some(ssh) = &config.ssh {
            // Extract remote DB host/port from driver params
            let (remote_host, remote_port) = extract_db_host_port(&config);
            match SshTunnel::establish(ssh, &remote_host, remote_port).await {
                Ok(t) => {
                    // Override config host/port to point through the tunnel
                    override_host_port(&mut tunnel_config, "127.0.0.1", t.local_addr.port());
                    Some(t)
                }
                Err(e) => {
                    self.send_error(conn_id, None, format!("SSH tunnel failed: {}", e))
                        .await;
                    return;
                }
            }
        } else {
            None
        };

        let mut driver = match DbFactory::create(&tunnel_config, self.gate.as_ref()) {
            Ok(d) => d,
            Err(e) => {
                self.send_error(conn_id, None, e.to_string()).await;
                return;
            }
        };
        if let Err(e) = driver.connect(&tunnel_config).await {
            self.send_error(conn_id, None, e.to_string()).await;
            return;
        }
        let db_result = driver.list_databases().await;

        // Query server version (best-effort — ignore errors)
        let server_version = match driver.execute("SELECT version()").await {
            Ok(qr) => qr
                .rows
                .first()
                .and_then(|row| row.first())
                .and_then(|v| match v {
                    crate::db::values::DbValue::Text(s) => Some(s.clone()),
                    _ => None,
                }),
            Err(_) => None,
        };

        self.connections.insert(conn_id, driver);
        if let Some(t) = tunnel {
            self.tunnels.insert(conn_id, t);
        }
        match db_result {
            Ok(databases) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::Connected {
                        conn_id,
                        databases,
                        server_version,
                    })
                    .await;
            }
            Err(e) => {
                self.send_error(
                    conn_id,
                    None,
                    format!("Connected but listing databases failed: {}", e),
                )
                .await;
            }
        }
    }

    /// Test a connection without persisting it — connect, then immediately disconnect.
    pub(super) async fn handle_test_connection(
        &mut self,
        config: crate::db::connection::ConnectionConfig,
    ) {
        // Establish SSH tunnel if configured
        let mut tunnel_config = config.clone();
        let _tunnel = if let Some(ssh) = &config.ssh {
            let (remote_host, remote_port) = extract_db_host_port(&config);
            match SshTunnel::establish(ssh, &remote_host, remote_port).await {
                Ok(t) => {
                    override_host_port(&mut tunnel_config, "127.0.0.1", t.local_addr.port());
                    Some(t)
                }
                Err(e) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::TestConnectionResult {
                            success: false,
                            message: format!("SSH tunnel failed: {}", e),
                        })
                        .await;
                    return;
                }
            }
        } else {
            None
        };

        let mut driver = match DbFactory::create(&tunnel_config, self.gate.as_ref()) {
            Ok(d) => d,
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::TestConnectionResult {
                        success: false,
                        message: e.to_string(),
                    })
                    .await;
                return;
            }
        };
        if let Err(e) = driver.connect(&tunnel_config).await {
            let _ = self
                .event_tx
                .send(DbEvent::TestConnectionResult {
                    success: false,
                    message: e.to_string(),
                })
                .await;
            return;
        }
        // Success — disconnect immediately (tunnel drops automatically)
        let _ = driver.disconnect().await;
        let _ = self
            .event_tx
            .send(DbEvent::TestConnectionResult {
                success: true,
                message: "Connection successful!".to_string(),
            })
            .await;
    }

    pub(super) async fn handle_disconnect(&mut self, conn_id: Uuid) {
        if let Some(mut driver) = self.connections.remove(&conn_id) {
            let _ = driver.disconnect().await;
        }
        // Tear down SSH tunnel if one exists
        if let Some(tunnel) = self.tunnels.remove(&conn_id) {
            tunnel.close();
        }
        let _ = self
            .event_tx
            .send(DbEvent::Disconnected { conn_id })
            .await;
    }
}

/// Extract the remote DB host/port from a ConnectionConfig's driver params.
fn extract_db_host_port(config: &crate::db::connection::ConnectionConfig) -> (String, u16) {
    match &config.params {
        crate::db::connection::DriverParams::Postgres { host, port, .. }
        | crate::db::connection::DriverParams::Mysql { host, port, .. }
        | crate::db::connection::DriverParams::Mssql { host, port, .. } => {
            (host.clone(), *port)
        }
        crate::db::connection::DriverParams::Redis { host, port, .. } => {
            (host.clone(), *port)
        }
        _ => ("localhost".to_string(), 5432),
    }
}

/// Override host/port in a ConnectionConfig's driver params to point through the tunnel.
fn override_host_port(
    config: &mut crate::db::connection::ConnectionConfig,
    host: &str,
    port: u16,
) {
    match &mut config.params {
        crate::db::connection::DriverParams::Postgres {
            host: h, port: p, ..
        }
        | crate::db::connection::DriverParams::Mysql {
            host: h, port: p, ..
        }
        | crate::db::connection::DriverParams::Mssql {
            host: h, port: p, ..
        } => {
            *h = host.to_string();
            *p = port;
        }
        crate::db::connection::DriverParams::Redis {
            host: h, port: p, ..
        } => {
            *h = host.to_string();
            *p = port;
        }
        _ => {}
    }
}
