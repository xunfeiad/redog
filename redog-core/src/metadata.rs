use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Tcp,
    Udp,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::Tcp => write!(f, "tcp"),
            Network::Udp => write!(f, "udp"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InboundType {
    Http,
    HttpConnect,
    Socks4,
    Socks5,
    Redir,
    TProxy,
    Tun,
    Mixed,
    Inner,
}

impl fmt::Display for InboundType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InboundType::Http => write!(f, "HTTP"),
            InboundType::HttpConnect => write!(f, "HTTP Connect"),
            InboundType::Socks4 => write!(f, "SOCKS4"),
            InboundType::Socks5 => write!(f, "SOCKS5"),
            InboundType::Redir => write!(f, "Redir"),
            InboundType::TProxy => write!(f, "TProxy"),
            InboundType::Tun => write!(f, "TUN"),
            InboundType::Mixed => write!(f, "Mixed"),
            InboundType::Inner => write!(f, "Inner"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DnsMode {
    Normal,
    FakeIp,
    RedirHost,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metadata {
    pub network: Network,
    pub inbound_type: InboundType,
    pub src_addr: SocketAddr,
    pub dst_ip: Option<IpAddr>,
    pub dst_port: u16,
    pub host: Option<String>,
    pub dns_mode: DnsMode,
    pub process_path: Option<String>,
    pub process_name: Option<String>,
    pub inbound_addr: Option<SocketAddr>,
    pub inbound_name: Option<String>,
    pub dscp: u8,
}

impl Metadata {
    pub fn new(network: Network, inbound_type: InboundType) -> Self {
        Self {
            network,
            inbound_type,
            src_addr: SocketAddr::from(([0, 0, 0, 0], 0)),
            dst_ip: None,
            dst_port: 0,
            host: None,
            dns_mode: DnsMode::Normal,
            process_path: None,
            process_name: None,
            inbound_addr: None,
            inbound_name: None,
            dscp: 0,
        }
    }

    /// Get the target host string (domain preferred, then IP)
    pub fn remote_host(&self) -> String {
        if let Some(ref host) = self.host {
            host.clone()
        } else if let Some(ip) = self.dst_ip {
            ip.to_string()
        } else {
            String::new()
        }
    }

    /// Get the target SocketAddr
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.dst_ip.map(|ip| SocketAddr::new(ip, self.dst_port))
    }

    /// Get display string for the destination
    pub fn destination(&self) -> String {
        let host = self.remote_host();
        if host.is_empty() {
            format!(":{}", self.dst_port)
        } else {
            format!("{}:{}", host, self.dst_port)
        }
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} -> {}",
            self.inbound_type,
            self.network,
            self.src_addr,
            self.destination()
        )
    }
}
