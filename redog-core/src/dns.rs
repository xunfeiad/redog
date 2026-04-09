use async_trait::async_trait;
use std::net::IpAddr;

use crate::error::Error;

/// DNS resolver trait — tunnel core interacts with DNS through this
#[async_trait]
pub trait DnsResolver: Send + Sync {
    /// Resolve domain to IPv4
    async fn resolve_v4(&self, host: &str) -> Result<IpAddr, Error>;

    /// Resolve domain to IPv6
    async fn resolve_v6(&self, host: &str) -> Result<IpAddr, Error>;

    /// Resolve domain (prefer v4)
    async fn resolve(&self, host: &str) -> Result<IpAddr, Error> {
        self.resolve_v4(host).await
    }

    /// FakeIP reverse lookup: IP -> domain
    async fn fake_ip_lookup(&self, ip: IpAddr) -> Option<String>;

    /// Check if an IP is a FakeIP
    fn is_fake_ip(&self, ip: IpAddr) -> bool;

    /// Check if FakeIP mode is enabled
    fn is_fake_ip_enabled(&self) -> bool {
        false
    }
}
