use redog_core::metadata::Metadata;
use redog_core::rule::{Rule, RuleType};

/// Exact domain match
#[derive(Debug)]
pub struct DomainRule {
    pub domain: String,
    pub adapter: String,
}

impl Rule for DomainRule {
    fn rule_type(&self) -> RuleType {
        RuleType::Domain
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        metadata
            .host
            .as_deref()
            .map(|h| h.eq_ignore_ascii_case(&self.domain))
            .unwrap_or(false)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        &self.domain
    }
}

/// Domain suffix match
#[derive(Debug)]
pub struct DomainSuffixRule {
    pub suffix: String,
    pub adapter: String,
}

impl Rule for DomainSuffixRule {
    fn rule_type(&self) -> RuleType {
        RuleType::DomainSuffix
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        metadata
            .host
            .as_deref()
            .map(|h| {
                let h = h.to_lowercase();
                let s = self.suffix.to_lowercase();
                h == s || h.ends_with(&format!(".{}", s))
            })
            .unwrap_or(false)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        &self.suffix
    }
}

/// Domain keyword match
#[derive(Debug)]
pub struct DomainKeywordRule {
    pub keyword: String,
    pub adapter: String,
}

impl Rule for DomainKeywordRule {
    fn rule_type(&self) -> RuleType {
        RuleType::DomainKeyword
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        metadata
            .host
            .as_deref()
            .map(|h| h.to_lowercase().contains(&self.keyword.to_lowercase()))
            .unwrap_or(false)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        &self.keyword
    }
}

/// Domain regex match
#[derive(Debug)]
pub struct DomainRegexRule {
    pub pattern: regex::Regex,
    pub pattern_str: String,
    pub adapter: String,
}

impl Rule for DomainRegexRule {
    fn rule_type(&self) -> RuleType {
        RuleType::DomainRegex
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        metadata
            .host
            .as_deref()
            .map(|h| self.pattern.is_match(h))
            .unwrap_or(false)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn payload(&self) -> &str {
        &self.pattern_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redog_core::metadata::{InboundType, Network};

    fn make_meta(host: &str) -> Metadata {
        let mut m = Metadata::new(Network::Tcp, InboundType::Http);
        m.host = Some(host.to_string());
        m
    }

    #[test]
    fn test_domain_rule() {
        let rule = DomainRule {
            domain: "google.com".into(),
            adapter: "Proxy".into(),
        };
        assert!(rule.matches(&make_meta("google.com")));
        assert!(!rule.matches(&make_meta("www.google.com")));
    }

    #[test]
    fn test_domain_suffix() {
        let rule = DomainSuffixRule {
            suffix: "google.com".into(),
            adapter: "Proxy".into(),
        };
        assert!(rule.matches(&make_meta("google.com")));
        assert!(rule.matches(&make_meta("www.google.com")));
        assert!(rule.matches(&make_meta("a.b.google.com")));
        assert!(!rule.matches(&make_meta("notgoogle.com")));
    }

    #[test]
    fn test_domain_keyword() {
        let rule = DomainKeywordRule {
            keyword: "google".into(),
            adapter: "Proxy".into(),
        };
        assert!(rule.matches(&make_meta("www.google.com")));
        assert!(rule.matches(&make_meta("google.co.jp")));
        assert!(!rule.matches(&make_meta("baidu.com")));
    }
}
