/// Sniff TLS ClientHello to extract SNI
pub fn sniff_tls_sni(data: &[u8]) -> Option<String> {
    // Minimum TLS record: 5 byte header + 1 byte handshake
    if data.len() < 6 {
        return None;
    }

    // TLS Record Header
    if data[0] != 0x16 {
        // Not a Handshake record
        return None;
    }

    // TLS version check (0x0301 = TLS 1.0, 0x0303 = TLS 1.2)
    // TLS 1.3 still uses 0x0301 in record layer
    let _record_version = ((data[1] as u16) << 8) | (data[2] as u16);
    let record_length = ((data[3] as usize) << 8) | (data[4] as usize);

    if data.len() < 5 + record_length {
        return None;
    }

    let handshake = &data[5..5 + record_length];
    parse_client_hello_sni(handshake)
}

fn parse_client_hello_sni(data: &[u8]) -> Option<String> {
    if data.is_empty() || data[0] != 0x01 {
        // Not ClientHello
        return None;
    }

    if data.len() < 4 {
        return None;
    }

    let hello_length = ((data[1] as usize) << 16) | ((data[2] as usize) << 8) | (data[3] as usize);
    if data.len() < 4 + hello_length {
        return None;
    }

    let mut pos = 4;

    // Client version (2 bytes)
    pos += 2;
    // Random (32 bytes)
    pos += 32;

    if pos >= data.len() {
        return None;
    }

    // Session ID
    let session_id_len = data[pos] as usize;
    pos += 1 + session_id_len;

    if pos + 2 > data.len() {
        return None;
    }

    // Cipher Suites
    let cipher_suites_len = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
    pos += 2 + cipher_suites_len;

    if pos >= data.len() {
        return None;
    }

    // Compression Methods
    let compression_len = data[pos] as usize;
    pos += 1 + compression_len;

    if pos + 2 > data.len() {
        return None;
    }

    // Extensions
    let extensions_len = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
    pos += 2;

    let extensions_end = pos + extensions_len;
    if extensions_end > data.len() {
        return None;
    }

    // Iterate extensions
    while pos + 4 <= extensions_end {
        let ext_type = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
        let ext_len = ((data[pos + 2] as usize) << 8) | (data[pos + 3] as usize);
        pos += 4;

        if ext_type == 0x0000 {
            // SNI extension
            return parse_sni_extension(&data[pos..pos + ext_len]);
        }

        pos += ext_len;
    }

    None
}

fn parse_sni_extension(data: &[u8]) -> Option<String> {
    if data.len() < 2 {
        return None;
    }

    let list_len = ((data[0] as usize) << 8) | (data[1] as usize);
    if data.len() < 2 + list_len {
        return None;
    }

    let mut pos = 2;
    while pos + 3 <= 2 + list_len {
        let name_type = data[pos];
        let name_len = ((data[pos + 1] as usize) << 8) | (data[pos + 2] as usize);
        pos += 3;

        if name_type == 0x00 {
            // Host name
            if pos + name_len <= data.len() {
                return String::from_utf8(data[pos..pos + name_len].to_vec()).ok();
            }
        }

        pos += name_len;
    }

    None
}

/// Sniff HTTP Host header
pub fn sniff_http_host(data: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(data).ok()?;

    // Check if it looks like an HTTP request
    if !text.starts_with("GET ")
        && !text.starts_with("POST ")
        && !text.starts_with("PUT ")
        && !text.starts_with("DELETE ")
        && !text.starts_with("HEAD ")
        && !text.starts_with("OPTIONS ")
        && !text.starts_with("CONNECT ")
        && !text.starts_with("PATCH ")
    {
        return None;
    }

    // Find Host header
    for line in text.lines() {
        if let Some(host) = line.strip_prefix("Host: ").or_else(|| line.strip_prefix("host: ")) {
            let host = host.trim();
            // Remove port if present
            if let Some(colon_pos) = host.rfind(':') {
                if host[colon_pos + 1..].parse::<u16>().is_ok() {
                    return Some(host[..colon_pos].to_string());
                }
            }
            return Some(host.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniff_http_host() {
        let data = b"GET / HTTP/1.1\r\nHost: www.google.com\r\nConnection: close\r\n\r\n";
        assert_eq!(sniff_http_host(data), Some("www.google.com".to_string()));

        let data = b"GET / HTTP/1.1\r\nHost: example.com:8080\r\n\r\n";
        assert_eq!(sniff_http_host(data), Some("example.com".to_string()));
    }
}
