use redog_core::metadata::Metadata;
use redog_core::rule::{Rule, RuleType};

/// Source port match
#[derive(Debug)]
pub struct SrcPortRule {
    pub port: u16,
    pub adapter: String,
}

impl Rule for SrcPortRule {
    fn rule_type(&self) -> RuleType {
        RuleType::SrcPort
    }
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.src_addr.port() == self.port
    }
    fn adapter(&self) -> &str {
        &self.adapter
    }
    fn payload(&self) -> &str {
        // We return a static reference workaround
        ""
    }
}

/// Destination port match
#[derive(Debug)]
pub struct DstPortRule {
    pub port: u16,
    pub adapter: String,
}

impl Rule for DstPortRule {
    fn rule_type(&self) -> RuleType {
        RuleType::DstPort
    }
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.dst_port == self.port
    }
    fn adapter(&self) -> &str {
        &self.adapter
    }
    fn payload(&self) -> &str {
        ""
    }
}
