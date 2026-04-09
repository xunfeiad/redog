use async_trait::async_trait;
use std::net::SocketAddr;

use redog_core::adapter::{AdapterType, ProxyAdapter};
use redog_core::conn::{ProxyDatagram, ProxyStream};
use redog_core::error::Error;
use redog_core::metadata::Metadata;

/// Reject adapter — drops connections
#[derive(Debug)]
pub struct Reject;

impl Reject {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProxyAdapter for Reject {
    fn name(&self) -> &str {
        "REJECT"
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Reject
    }

    async fn connect_stream(&self, _metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        Err(Error::ConnectionClosed)
    }

    async fn connect_datagram(
        &self,
        _metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error> {
        Err(Error::ConnectionClosed)
    }
}
