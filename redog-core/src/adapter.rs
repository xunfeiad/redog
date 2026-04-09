use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::conn::{ProxyDatagram, ProxyStream};
use crate::error::Error;
use crate::metadata::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdapterType {
    Direct,
    Reject,
    Shadowsocks,
    VMess,
    VLESS,
    Trojan,
    WireGuard,
    Hysteria2,
    Http,
    Socks5,
    // Proxy groups
    Selector,
    URLTest,
    Fallback,
    LoadBalance,
    Relay,
}

impl fmt::Display for AdapterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterType::Direct => write!(f, "Direct"),
            AdapterType::Reject => write!(f, "Reject"),
            AdapterType::Shadowsocks => write!(f, "Shadowsocks"),
            AdapterType::VMess => write!(f, "VMess"),
            AdapterType::VLESS => write!(f, "VLESS"),
            AdapterType::Trojan => write!(f, "Trojan"),
            AdapterType::WireGuard => write!(f, "WireGuard"),
            AdapterType::Hysteria2 => write!(f, "Hysteria2"),
            AdapterType::Http => write!(f, "Http"),
            AdapterType::Socks5 => write!(f, "Socks5"),
            AdapterType::Selector => write!(f, "Selector"),
            AdapterType::URLTest => write!(f, "URLTest"),
            AdapterType::Fallback => write!(f, "Fallback"),
            AdapterType::LoadBalance => write!(f, "LoadBalance"),
            AdapterType::Relay => write!(f, "Relay"),
        }
    }
}

/// Core proxy adapter trait.
/// All proxy nodes and proxy groups implement this trait.
#[async_trait]
pub trait ProxyAdapter: Send + Sync + fmt::Debug {
    /// Adapter name
    fn name(&self) -> &str;

    /// Adapter type
    fn adapter_type(&self) -> AdapterType;

    /// Establish a TCP connection through this proxy
    async fn connect_stream(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyStream>, Error>;

    /// Establish a UDP session through this proxy
    async fn connect_datagram(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error>;

    /// Whether this adapter supports UDP
    fn support_udp(&self) -> bool {
        false
    }

    /// The proxy server address (for single proxies)
    fn addr(&self) -> Option<SocketAddr> {
        None
    }

    /// Whether this adapter is alive (for health checking)
    fn alive(&self) -> bool {
        true
    }

    /// Unwrap inner adapter (for proxy groups)
    fn unwrap_adapter(&self) -> Option<Arc<dyn ProxyAdapter>> {
        None
    }
}
