use async_trait::async_trait;
use redog_component::fakeip::FakeIpPool;
use redog_core::dns::DnsResolver;
use redog_core::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

/// System DNS resolver using tokio's built-in resolver
#[derive(Debug)]
pub struct SystemResolver {
    fakeip_pool: Option<Arc<FakeIpPool>>,
}

impl SystemResolver {
    pub fn new() -> Self {
        Self { fakeip_pool: None }
    }

    pub fn with_fakeip(pool: Arc<FakeIpPool>) -> Self {
        Self {
            fakeip_pool: Some(pool),
        }
    }
}

#[async_trait]
impl DnsResolver for SystemResolver {
    async fn resolve_v4(&self, host: &str) -> Result<IpAddr, Error> {
        let addr_str = format!("{}:0", host);
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(|e| Error::DnsResolution(format!("{}: {}", host, e)))?
            .collect();

        addrs
            .iter()
            .find(|a| a.is_ipv4())
            .map(|a| a.ip())
            .or_else(|| addrs.first().map(|a| a.ip()))
            .ok_or_else(|| Error::DnsResolution(format!("no addresses for {}", host)))
    }

    async fn resolve_v6(&self, host: &str) -> Result<IpAddr, Error> {
        let addr_str = format!("{}:0", host);
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(|e| Error::DnsResolution(format!("{}: {}", host, e)))?
            .collect();

        addrs
            .iter()
            .find(|a| a.is_ipv6())
            .map(|a| a.ip())
            .ok_or_else(|| Error::DnsResolution(format!("no IPv6 addresses for {}", host)))
    }

    async fn fake_ip_lookup(&self, ip: IpAddr) -> Option<String> {
        if let Some(ref pool) = self.fakeip_pool {
            if let IpAddr::V4(v4) = ip {
                return pool.reverse_lookup(v4);
            }
        }
        None
    }

    fn is_fake_ip(&self, ip: IpAddr) -> bool {
        self.fakeip_pool
            .as_ref()
            .map(|p| p.contains(ip))
            .unwrap_or(false)
    }

    fn is_fake_ip_enabled(&self) -> bool {
        self.fakeip_pool.is_some()
    }
}
