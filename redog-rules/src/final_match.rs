use redog_core::metadata::Metadata;
use redog_core::rule::{Rule, RuleType};

/// MATCH rule — catch-all default rule
#[derive(Debug)]
pub struct MatchRule {
    pub adapter: String,
}

impl Rule for MatchRule {
    fn rule_type(&self) -> RuleType {
        RuleType::Match
    }

    fn matches(&self, _metadata: &Metadata) -> bool {
        true // Always matches
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        ""
    }
}
