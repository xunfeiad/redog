use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::{TcpStream, UdpSocket};

use redog_core::adapter::{AdapterType, ProxyAdapter};
use redog_core::conn::{ProxyDatagram, ProxyStream, TrackedDatagram, TrackedStream};
use redog_core::error::Error;
use redog_core::metadata::Metadata;

/// Direct adapter — connects directly to the target
#[derive(Debug)]
pub struct Direct;

impl Direct {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProxyAdapter for Direct {
    fn name(&self) -> &str {
        "DIRECT"
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Direct
    }

    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        let addr = resolve_addr(metadata).await?;

        tracing::debug!("DIRECT connecting to {}", addr);
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;

        let mut tracked = TrackedStream::with_chain(stream, "DIRECT".to_string());
        Ok(Box::new(tracked))
    }

    async fn connect_datagram(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        if let Some(addr) = metadata.remote_addr() {
            socket.connect(addr).await?;
        }
        Ok(Box::new(TrackedDatagram::new(socket)))
    }

    fn support_udp(&self) -> bool {
        true
    }
}

/// Resolve metadata to a SocketAddr for direct connection
async fn resolve_addr(metadata: &Metadata) -> Result<SocketAddr, Error> {
    if let Some(addr) = metadata.remote_addr() {
        return Ok(addr);
    }

    if let Some(ref host) = metadata.host {
        // Use tokio's built-in DNS resolution
        let addr_str = format!("{}:{}", host, metadata.dst_port);
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await?
            .collect();

        addrs
            .into_iter()
            .next()
            .ok_or_else(|| Error::DnsResolution(format!("no addresses for {}", host)))
    } else {
        Err(Error::InvalidAddress("no host or IP in metadata".into()))
    }
}
