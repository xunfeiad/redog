use serde::{Deserialize, Serialize};
use std::fmt;

use crate::metadata::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleType {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
    IpCidr,
    IpCidr6,
    SrcIpCidr,
    GeoIP,
    GeoSite,
    ProcessName,
    ProcessPath,
    SrcPort,
    DstPort,
    InboundPort,
    And,
    Or,
    Not,
    RuleSet,
    Match,
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleType::Domain => write!(f, "DOMAIN"),
            RuleType::DomainSuffix => write!(f, "DOMAIN-SUFFIX"),
            RuleType::DomainKeyword => write!(f, "DOMAIN-KEYWORD"),
            RuleType::DomainRegex => write!(f, "DOMAIN-REGEX"),
            RuleType::IpCidr => write!(f, "IP-CIDR"),
            RuleType::IpCidr6 => write!(f, "IP-CIDR6"),
            RuleType::SrcIpCidr => write!(f, "SRC-IP-CIDR"),
            RuleType::GeoIP => write!(f, "GEOIP"),
            RuleType::GeoSite => write!(f, "GEOSITE"),
            RuleType::ProcessName => write!(f, "PROCESS-NAME"),
            RuleType::ProcessPath => write!(f, "PROCESS-PATH"),
            RuleType::SrcPort => write!(f, "SRC-PORT"),
            RuleType::DstPort => write!(f, "DST-PORT"),
            RuleType::InboundPort => write!(f, "IN-PORT"),
            RuleType::And => write!(f, "AND"),
            RuleType::Or => write!(f, "OR"),
            RuleType::Not => write!(f, "NOT"),
            RuleType::RuleSet => write!(f, "RULE-SET"),
            RuleType::Match => write!(f, "MATCH"),
        }
    }
}

/// Rule trait — all rule types implement this
pub trait Rule: Send + Sync + fmt::Debug {
    /// The rule type
    fn rule_type(&self) -> RuleType;

    /// Check if the metadata matches this rule
    fn matches(&self, metadata: &Metadata) -> bool;

    /// The adapter name to use when matched
    fn adapter(&self) -> &str;

    /// The rule payload (domain, CIDR, etc.)
    fn payload(&self) -> &str;

    /// Whether DNS resolution is needed before matching
    fn should_resolve_ip(&self) -> bool {
        false
    }

    /// Whether process lookup is needed before matching
    fn should_find_process(&self) -> bool {
        false
    }
}
