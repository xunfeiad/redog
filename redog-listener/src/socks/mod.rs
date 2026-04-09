use redog_core::error::Error;
use redog_core::metadata::{InboundType, Metadata, Network};
use redog_transport::socks5;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tracing;

use crate::InboundConnection;

/// Start a SOCKS5 listener
pub async fn start_socks_listener(
    bind_addr: SocketAddr,
    sender: tokio::sync::mpsc::Sender<InboundConnection>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("SOCKS5 listener started on {}", bind_addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let sender = sender.clone();

        tokio::spawn(async move {
            match handle_socks_connection(stream, peer_addr).await {
                Ok(conn) => {
                    if sender.send(conn).await.is_err() {
                        tracing::warn!("tunnel channel closed");
                    }
                }
                Err(e) => {
                    tracing::debug!("SOCKS5 handshake failed from {}: {}", peer_addr, e);
                }
            }
        });
    }
}

async fn handle_socks_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
) -> Result<InboundConnection, Error> {
    let (cmd, addr) = socks5::server_handshake(&mut stream).await?;

    if cmd != socks5::Command::Connect {
        return Err(Error::Protocol(format!(
            "unsupported SOCKS5 command: {:?}",
            cmd
        )));
    }

    let mut metadata = Metadata::new(Network::Tcp, InboundType::Socks5);
    metadata.src_addr = peer_addr;
    metadata.host = Some(addr.host());
    metadata.dst_port = addr.port();
    metadata.dst_ip = addr.ip();

    Ok(InboundConnection { stream, metadata })
}
