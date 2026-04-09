use redog_core::metadata::Metadata;
use redog_core::rule::{Rule, RuleType};
use ipnet::IpNet;
use std::net::IpAddr;

/// IP CIDR match rule
#[derive(Debug)]
pub struct IpCidrRule {
    pub cidr: IpNet,
    pub cidr_str: String,
    pub adapter: String,
    pub is_src: bool,
}

impl IpCidrRule {
    pub fn new(cidr_str: &str, adapter: &str, is_src: bool) -> Result<Self, String> {
        let cidr: IpNet = cidr_str
            .parse()
            .map_err(|e| format!("invalid CIDR '{}': {}", cidr_str, e))?;
        Ok(Self {
            cidr,
            cidr_str: cidr_str.to_string(),
            adapter: adapter.to_string(),
            is_src,
        })
    }
}

impl Rule for IpCidrRule {
    fn rule_type(&self) -> RuleType {
        if self.is_src {
            RuleType::SrcIpCidr
        } else {
            match self.cidr {
                IpNet::V4(_) => RuleType::IpCidr,
                IpNet::V6(_) => RuleType::IpCidr6,
            }
        }
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        let ip: Option<IpAddr> = if self.is_src {
            Some(metadata.src_addr.ip())
        } else {
            metadata.dst_ip
        };

        ip.map(|ip| self.cidr.contains(&ip)).unwrap_or(false)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        &self.cidr_str
    }

    fn should_resolve_ip(&self) -> bool {
        !self.is_src
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redog_core::metadata::{InboundType, Network};
    use std::net::Ipv4Addr;

    #[test]
    fn test_ipcidr() {
        let rule = IpCidrRule::new("192.168.0.0/16", "DIRECT", false).unwrap();

        let mut m = Metadata::new(Network::Tcp, InboundType::Http);
        m.dst_ip = Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(rule.matches(&m));

        m.dst_ip = Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(!rule.matches(&m));
    }
}
