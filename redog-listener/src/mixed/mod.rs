use redog_core::error::Error;
use redog_core::metadata::{InboundType, Metadata, Network};
use redog_transport::socks5;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::InboundConnection;

/// Start a mixed listener (HTTP + SOCKS5 on the same port)
/// Detects protocol by peeking the first byte
pub async fn start_mixed_listener(
    bind_addr: SocketAddr,
    sender: tokio::sync::mpsc::Sender<InboundConnection>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("Mixed (HTTP+SOCKS5) listener started on {}", bind_addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let sender = sender.clone();

        tokio::spawn(async move {
            match handle_mixed_connection(stream, peer_addr).await {
                Ok(conn) => {
                    if sender.send(conn).await.is_err() {
                        tracing::warn!("tunnel channel closed");
                    }
                }
                Err(e) => {
                    tracing::debug!("Mixed handler failed from {}: {}", peer_addr, e);
                }
            }
        });
    }
}

async fn handle_mixed_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> Result<InboundConnection, Error> {
    // Peek the first byte to determine protocol
    let mut peek_buf = [0u8; 1];
    stream.peek(&mut peek_buf).await?;

    if peek_buf[0] == 0x05 {
        // SOCKS5
        handle_socks5(stream, peer_addr).await
    } else {
        // HTTP
        handle_http(stream, peer_addr).await
    }
}

async fn handle_socks5(
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

async fn handle_http(
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> Result<InboundConnection, Error> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return Err(Error::Protocol("invalid HTTP request".into()));
    }

    let method = parts[0];
    let target = parts[1];

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = parse_connect_target(target)?;

        // Read remaining headers
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line.trim().is_empty() {
                break;
            }
        }

        let mut stream = reader.into_inner();
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        let mut metadata = Metadata::new(Network::Tcp, InboundType::HttpConnect);
        metadata.src_addr = peer_addr;
        metadata.host = Some(host);
        metadata.dst_port = port;

        Ok(InboundConnection { stream, metadata })
    } else {
        // Plain HTTP
        let host_port = if target.starts_with("http://") {
            let url_part = &target[7..];
            let end = url_part.find('/').unwrap_or(url_part.len());
            url_part[..end].to_string()
        } else {
            target.to_string()
        };

        let (host, port) = parse_connect_target(&host_port).unwrap_or((host_port, 80));

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line.trim().is_empty() {
                break;
            }
        }

        let mut metadata = Metadata::new(Network::Tcp, InboundType::Http);
        metadata.src_addr = peer_addr;
        metadata.host = Some(host);
        metadata.dst_port = port;

        let stream = reader.into_inner();
        Ok(InboundConnection { stream, metadata })
    }
}

fn parse_connect_target(s: &str) -> Result<(String, u16), Error> {
    if let Some(colon) = s.rfind(':') {
        let host = s[..colon].to_string();
        let port: u16 = s[colon + 1..]
            .parse()
            .map_err(|_| Error::Protocol(format!("invalid port in: {}", s)))?;
        Ok((host, port))
    } else {
        Err(Error::Protocol(format!("no port in: {}", s)))
    }
}
