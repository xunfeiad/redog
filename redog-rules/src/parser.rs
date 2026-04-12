use redog_core::rule::Rule;

use crate::domain::{DomainKeywordRule, DomainRegexRule, DomainRule, DomainSuffixRule};
use crate::final_match::MatchRule;
use crate::ipcidr::IpCidrRule;
use crate::port::{DstPortRule, SrcPortRule};

/// Parse a rule string like "DOMAIN-SUFFIX,google.com,Proxy"
pub fn parse_rule(line: &str) -> Result<Box<dyn Rule>, String> {
    let line = line.trim();

    // MATCH only has one comma: "MATCH,adapter"
    if line.starts_with("MATCH,") || line == "MATCH" {
        let adapter = line.strip_prefix("MATCH,").unwrap_or("DIRECT").trim();
        return Ok(Box::new(MatchRule {
            adapter: adapter.to_string(),
        }));
    }

    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.len() < 3 {
        return Err(format!("invalid rule format: {}", line));
    }

    let rule_type = parts[0].trim();
    let payload = parts[1].trim();
    let adapter = parts[2].trim().to_string();

    match rule_type {
        "DOMAIN" => Ok(Box::new(DomainRule {
            domain: payload.to_string(),
            adapter,
        })),
        "DOMAIN-SUFFIX" => Ok(Box::new(DomainSuffixRule {
            suffix: payload.to_string(),
            adapter,
        })),
        "DOMAIN-KEYWORD" => Ok(Box::new(DomainKeywordRule {
            keyword: payload.to_string(),
            adapter,
        })),
        "DOMAIN-REGEX" => {
            let pattern = regex::Regex::new(payload)
                .map_err(|e| format!("invalid regex '{}': {}", payload, e))?;
            Ok(Box::new(DomainRegexRule {
                pattern,
                pattern_str: payload.to_string(),
                adapter,
            }))
        }
        "IP-CIDR" | "IP-CIDR6" => {
            let rule = IpCidrRule::new(payload, &adapter, false)?;
            Ok(Box::new(rule))
        }
        "SRC-IP-CIDR" => {
            let rule = IpCidrRule::new(payload, &adapter, true)?;
            Ok(Box::new(rule))
        }
        "SRC-PORT" => {
            let port: u16 = payload
                .parse()
                .map_err(|e| format!("invalid port '{}': {}", payload, e))?;
            Ok(Box::new(SrcPortRule::new(port, adapter)))
        }
        "DST-PORT" => {
            let port: u16 = payload
                .parse()
                .map_err(|e| format!("invalid port '{}': {}", payload, e))?;
            Ok(Box::new(DstPortRule::new(port, adapter)))
        }
        _ => Err(format!("unknown rule type: {}", rule_type)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rules() {
        assert!(parse_rule("DOMAIN,google.com,Proxy").is_ok());
        assert!(parse_rule("DOMAIN-SUFFIX,google.com,Proxy").is_ok());
        assert!(parse_rule("DOMAIN-KEYWORD,google,Proxy").is_ok());
        assert!(parse_rule("IP-CIDR,192.168.0.0/16,DIRECT").is_ok());
        assert!(parse_rule("MATCH,Proxy").is_ok());
        assert!(parse_rule("INVALID").is_err());
    }
}
