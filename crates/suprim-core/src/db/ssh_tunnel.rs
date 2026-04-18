//! SSH tunnel — establishes a local TCP port forwarding through an SSH connection.
//!
//! Uses `russh` for async SSH and spawns a background task to proxy traffic between
//! the local listener and the SSH `direct-tcpip` channel.

use std::net::SocketAddr;
use std::sync::Arc;

use russh::client::AuthResult;
use russh::keys::ssh_key;
use russh::{client, Channel, ChannelMsg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Notify;

use crate::db::connection::SshConfig;
use crate::error::Result;

/// A running SSH tunnel. Drop this to tear down the tunnel.
pub struct SshTunnel {
    /// Local address the tunnel is listening on (127.0.0.1:auto_port).
    pub local_addr: SocketAddr,
    /// Signal to stop the background listener task.
    shutdown: Arc<Notify>,
}

impl SshTunnel {
    /// Establish an SSH tunnel.
    ///
    /// - Connects to the SSH server at `ssh.host:ssh.port`.
    /// - Authenticates with key file or password.
    /// - Binds a local TCP listener on `127.0.0.1:0` (OS-assigned port).
    /// - Each incoming connection gets forwarded to `remote_host:remote_port`
    ///   via the SSH `direct-tcpip` channel.
    pub async fn establish(
        ssh: &SshConfig,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<Self> {
        // 1. Build SSH client config
        let config = Arc::new(client::Config::default());
        let handler = SshHandler;
        let mut session =
            client::connect(config, (ssh.host.as_str(), ssh.port), handler).await?;

        // 2. Authenticate
        let auth_result = if let Some(key_path) = &ssh.key_path {
            // load_secret_key handles all formats: OpenSSH, PEM/PKCS1, PKCS8, PuTTY
            let key_pair = russh::keys::load_secret_key(key_path, None).map_err(|e| {
                crate::error::AppError::Ssh(format!(
                    "Failed to load SSH key {}: {}",
                    key_path.display(),
                    e
                ))
            })?;
            // RSA keys need SHA-256 hash (SHA-1 "ssh-rsa" rejected by modern servers)
            let hash_alg = if matches!(key_pair.algorithm(), ssh_key::Algorithm::Rsa { .. }) {
                Some(ssh_key::HashAlg::Sha256)
            } else {
                None
            };
            let key_with_alg =
                russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg);
            session
                .authenticate_publickey(&ssh.user, key_with_alg)
                .await?
        } else if let Some(password) = &ssh.password_key {
            let decrypted = crate::storage::credential::decrypt(password);
            session.authenticate_password(&ssh.user, &decrypted).await?
        } else {
            return Err(crate::error::AppError::Ssh(
                "No SSH authentication method provided (key or password)".into(),
            ));
        };

        if !matches!(auth_result, AuthResult::Success) {
            return Err(crate::error::AppError::Ssh(
                "SSH authentication failed".into(),
            ));
        }

        // 3. Bind local listener
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;

        // 4. Spawn background proxy task
        let shutdown = Arc::new(Notify::new());
        let shutdown_clone = shutdown.clone();
        let rh = remote_host.to_string();
        let rp = remote_port;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_clone.notified() => break,
                    accept = listener.accept() => {
                        match accept {
                            Ok((tcp_stream, _)) => {
                                // Open a direct-tcpip channel for each connection
                                let channel = match session
                                    .channel_open_direct_tcpip(
                                        rh.clone(),
                                        rp as u32,
                                        "127.0.0.1",
                                        0,
                                    )
                                    .await
                                {
                                    Ok(ch) => ch,
                                    Err(e) => {
                                        tracing::warn!("SSH tunnel channel open error: {}", e);
                                        continue;
                                    }
                                };

                                tokio::spawn(async move {
                                    if let Err(e) = proxy_connection(channel, tcp_stream).await {
                                        tracing::warn!("SSH tunnel proxy error: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::warn!("SSH tunnel accept error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(SshTunnel {
            local_addr,
            shutdown,
        })
    }

    /// Shut down the tunnel: stops the listener and drops the SSH session.
    pub fn close(&self) {
        self.shutdown.notify_one();
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.shutdown.notify_one();
    }
}

/// Proxy data between a local TCP stream and an SSH direct-tcpip channel.
///
/// Uses `Channel.split()` to get a read half and write half, then
/// bidirectionally copies data between the TCP stream and the SSH channel.
async fn proxy_connection(
    channel: Channel<client::Msg>,
    tcp_stream: tokio::net::TcpStream,
) -> Result<()> {
    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();
    let (mut ch_read, ch_write) = channel.split();

    // TCP → SSH channel (via AsyncWrite on ChannelWriteHalf)
    let mut ch_writer = ch_write.make_writer();
    let tcp_to_ssh = tokio::spawn(async move {
        let mut buf = vec![0u8; 32768];
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ch_writer.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // SSH channel → TCP (via wait() on ChannelReadHalf)
    let ssh_to_tcp = tokio::spawn(async move {
        loop {
            match ch_read.wait().await {
                Some(ChannelMsg::Data { data })
                    if tcp_write.write_all(&data).await.is_err() =>
                {
                    break;
                }
                Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
    });

    let _ = tokio::join!(tcp_to_ssh, ssh_to_tcp);
    Ok(())
}

/// Minimal SSH client handler — accepts all server host keys.
/// TODO: implement known_hosts verification for production use.
struct SshHandler;

impl client::Handler for SshHandler {
    type Error = crate::error::AppError;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // Accept all host keys for now.
        // TODO: check against ~/.ssh/known_hosts
        Ok(true)
    }
}
