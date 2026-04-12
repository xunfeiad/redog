use redog_core::error::Error;

/// Parse a host:port string with IPv6 support
/// Returns (host, port). Uses default_port if no port is specified.
pub fn parse_host_port(s: &str, default_port: u16) -> Result<(String, u16), Error> {
    if s.is_empty() {
        return Err(Error::Protocol("empty host string".into()));
    }

    // IPv6: [::1]:port
    if let Some(bracket_end) = s.find(']') {
        if !s.starts_with('[') || bracket_end == 0 {
            return Err(Error::Protocol(format!("malformed IPv6 address: {}", s)));
        }
        let host = s[1..bracket_end].to_string();
        let rest = &s[bracket_end + 1..];
        let port = if let Some(port_str) = rest.strip_prefix(':') {
            if port_str.is_empty() {
                default_port
            } else {
                port_str
                    .parse()
                    .map_err(|_| Error::Protocol(format!("invalid port in: {}", s)))?
            }
        } else {
            default_port
        };
        Ok((host, port))
    } else if let Some(colon) = s.rfind(':') {
        // Ensure colon is not at the start or end
        let host = &s[..colon];
        let port_str = &s[colon + 1..];
        if host.is_empty() {
            return Err(Error::Protocol(format!("empty host in: {}", s)));
        }
        if port_str.is_empty() {
            // "host:" with trailing colon -> use default port
            Ok((host.to_string(), default_port))
        } else {
            let port: u16 = port_str
                .parse()
                .map_err(|_| Error::Protocol(format!("invalid port in: {}", s)))?;
            Ok((host.to_string(), port))
        }
    } else {
        Ok((s.to_string(), default_port))
    }
}

/// Extract host:port from an HTTP request target
/// Handles both absolute URLs (http://host:port/path) and authority form (host:port)
pub fn extract_host_from_target(target: &str, default_port: u16) -> Result<(String, u16), Error> {
    let host_port = if let Some(rest) = target.strip_prefix("http://") {
        let end = rest.find('/').unwrap_or(rest.len());
        &rest[..end]
    } else if let Some(rest) = target.strip_prefix("https://") {
        let end = rest.find('/').unwrap_or(rest.len());
        &rest[..end]
    } else {
        target
    };
    parse_host_port(host_port, default_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_port_basic() {
        assert_eq!(
            parse_host_port("example.com:8080", 80).unwrap(),
            ("example.com".to_string(), 8080)
        );
        assert_eq!(
            parse_host_port("example.com", 80).unwrap(),
            ("example.com".to_string(), 80)
        );
    }

    #[test]
    fn test_parse_host_port_ipv6() {
        assert_eq!(
            parse_host_port("[::1]:443", 80).unwrap(),
            ("::1".to_string(), 443)
        );
        assert_eq!(
            parse_host_port("[::1]", 80).unwrap(),
            ("::1".to_string(), 80)
        );
    }

    #[test]
    fn test_parse_host_port_trailing_colon() {
        assert_eq!(
            parse_host_port("example.com:", 80).unwrap(),
            ("example.com".to_string(), 80)
        );
    }

    #[test]
    fn test_parse_host_port_empty() {
        assert!(parse_host_port("", 80).is_err());
    }

    #[test]
    fn test_extract_host_from_url() {
        assert_eq!(
            extract_host_from_target("http://example.com:8080/path", 80).unwrap(),
            ("example.com".to_string(), 8080)
        );
        assert_eq!(
            extract_host_from_target("example.com:443", 443).unwrap(),
            ("example.com".to_string(), 443)
        );
    }
}
