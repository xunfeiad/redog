use redog_common::LruCache;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;

/// FakeIP address pool
/// Allocates virtual IPs from a private range and maintains bidirectional mapping
#[derive(Debug)]
pub struct FakeIpPool {
    ip_to_host: LruCache<Ipv4Addr, String>,
    host_to_ip: LruCache<String, Ipv4Addr>,
    /// Network base address
    network: u32,
    /// Network mask
    mask: u32,
    /// Pool size
    pool_size: u32,
    /// Current allocation offset
    offset: Mutex<u32>,
}

impl FakeIpPool {
    /// Create a new FakeIP pool from a CIDR range
    /// e.g. "198.18.0.0/15" gives 131072 addresses
    pub fn new(cidr: &str, capacity: usize) -> Result<Self, String> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid CIDR: {}", cidr));
        }

        let ip: Ipv4Addr = parts[0]
            .parse()
            .map_err(|e| format!("invalid IP: {}", e))?;
        let prefix_len: u32 = parts[1]
            .parse()
            .map_err(|e| format!("invalid prefix length: {}", e))?;

        let network = u32::from(ip);
        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };
        let pool_size = 1u32 << (32 - prefix_len);

        Ok(Self {
            ip_to_host: LruCache::new(capacity),
            host_to_ip: LruCache::new(capacity),
            network: network & mask,
            mask,
            pool_size,
            offset: Mutex::new(1), // skip network address
        })
    }

    /// Allocate a FakeIP for a domain (or return existing one)
    pub fn lookup(&self, host: &str) -> Ipv4Addr {
        // Check if already allocated
        if let Some(ip) = self.host_to_ip.get(&host.to_string()) {
            return ip;
        }

        // Allocate new IP
        let ip = self.allocate();
        self.ip_to_host.put(ip, host.to_string());
        self.host_to_ip.put(host.to_string(), ip);
        ip
    }

    /// Reverse lookup: IP -> domain
    pub fn reverse_lookup(&self, ip: Ipv4Addr) -> Option<String> {
        self.ip_to_host.get(&ip)
    }

    /// Check if an IP is in the FakeIP range
    pub fn contains(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let ip_num = u32::from(v4);
                (ip_num & self.mask) == self.network
            }
            IpAddr::V6(_) => false,
        }
    }

    fn allocate(&self) -> Ipv4Addr {
        let mut offset = self.offset.lock().unwrap();
        let ip_num = self.network + (*offset % (self.pool_size - 1)) + 1;
        *offset = offset.wrapping_add(1);
        Ipv4Addr::from(ip_num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fakeip_pool() {
        let pool = FakeIpPool::new("198.18.0.0/15", 1000).unwrap();

        let ip1 = pool.lookup("google.com");
        let ip2 = pool.lookup("facebook.com");
        let ip3 = pool.lookup("google.com"); // should return same

        assert_ne!(ip1, ip2);
        assert_eq!(ip1, ip3);
        assert!(pool.contains(IpAddr::V4(ip1)));
        assert!(pool.contains(IpAddr::V4(ip2)));
        assert_eq!(pool.reverse_lookup(ip1), Some("google.com".to_string()));
    }
}
