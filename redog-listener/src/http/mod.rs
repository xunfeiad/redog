use redog_core::error::Error;
use redog_core::metadata::{InboundType, Metadata, Network};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::InboundConnection;

/// Start an HTTP proxy listener
pub async fn start_http_listener(
    bind_addr: SocketAddr,
    sender: tokio::sync::mpsc::Sender<InboundConnection>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("HTTP listener started on {}", bind_addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let sender = sender.clone();

        tokio::spawn(async move {
            match handle_http_connection(stream, peer_addr).await {
                Ok(conn) => {
                    if sender.send(conn).await.is_err() {
                        tracing::warn!("tunnel channel closed");
                    }
                }
                Err(e) => {
                    tracing::debug!("HTTP proxy failed from {}: {}", peer_addr, e);
                }
            }
        });
    }
}

async fn handle_http_connection(
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
        // HTTPS CONNECT tunnel
        let (host, port) = parse_host_port(target, 443)?;

        // Read remaining headers (discard)
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line.trim().is_empty() {
                break;
            }
        }

        // Send 200 Connection Established
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
        // Plain HTTP proxy (GET, POST, etc.)
        // Parse Host header from the URL or headers
        let host_port = if target.starts_with("http://") {
            let url_part = &target[7..];
            let end = url_part.find('/').unwrap_or(url_part.len());
            &url_part[..end]
        } else {
            target
        };

        let (host, port) = parse_host_port(host_port, 80)?;

        // Read remaining headers
        let mut headers = request_line.clone();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            headers.push_str(&line);
            if line.trim().is_empty() {
                break;
            }
        }

        // For plain HTTP, we need to forward the original request
        // Return the stream with the request already buffered
        let mut metadata = Metadata::new(Network::Tcp, InboundType::Http);
        metadata.src_addr = peer_addr;
        metadata.host = Some(host);
        metadata.dst_port = port;

        let stream = reader.into_inner();
        Ok(InboundConnection { stream, metadata })
    }
}

fn parse_host_port(s: &str, default_port: u16) -> Result<(String, u16), Error> {
    if let Some(bracket_end) = s.find(']') {
        // IPv6: [::1]:port
        let host = s[1..bracket_end].to_string();
        let port = if s.len() > bracket_end + 2 && s.as_bytes()[bracket_end + 1] == b':' {
            s[bracket_end + 2..]
                .parse()
                .unwrap_or(default_port)
        } else {
            default_port
        };
        Ok((host, port))
    } else if let Some(colon) = s.rfind(':') {
        let host = s[..colon].to_string();
        let port = s[colon + 1..].parse().unwrap_or(default_port);
        Ok((host, port))
    } else {
        Ok((s.to_string(), default_port))
    }
}
