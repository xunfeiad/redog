use std::collections::HashMap;

/// Node in the domain trie
#[derive(Debug, Default)]
struct TrieNode {
    children: HashMap<String, TrieNode>,
    /// If this node terminates a rule, store the adapter name
    data: Option<String>,
}

/// Domain trie for O(k) suffix matching
/// Domains are inserted right-to-left by labels (e.g. "com" -> "google" -> "www")
#[derive(Debug, Default)]
pub struct DomainTrie {
    root: TrieNode,
}

impl DomainTrie {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a domain suffix rule
    /// e.g. insert_suffix("google.com", "Proxy") matches *.google.com and google.com
    pub fn insert_suffix(&mut self, domain: &str, data: String) {
        let labels: Vec<&str> = domain.split('.').rev().collect();
        let mut node = &mut self.root;
        for label in labels {
            node = node
                .children
                .entry(label.to_lowercase())
                .or_default();
        }
        node.data = Some(data);
    }

    /// Insert an exact domain rule
    pub fn insert_exact(&mut self, domain: &str, data: String) {
        // For exact match, we prefix with a special marker
        let key = format!("__exact__{}", domain.to_lowercase());
        let labels: Vec<&str> = domain.split('.').rev().collect();
        let mut node = &mut self.root;
        for label in labels {
            node = node
                .children
                .entry(label.to_lowercase())
                .or_default();
        }
        // Mark as exact (only matches this domain, not subdomains)
        node.children
            .entry("__exact__".to_string())
            .or_default()
            .data = Some(data);
    }

    /// Look up a domain, returning the matched data
    pub fn lookup(&self, domain: &str) -> Option<&str> {
        let labels: Vec<&str> = domain.split('.').rev().collect();
        let mut node = &self.root;
        let mut last_match: Option<&str> = None;

        for (i, label) in labels.iter().enumerate() {
            let lower = label.to_lowercase();
            match node.children.get(&lower) {
                Some(child) => {
                    node = child;
                    if let Some(ref data) = node.data {
                        last_match = Some(data.as_str());
                    }
                }
                None => break,
            }
        }

        last_match
    }

    /// Check if a domain matches any rule in the trie
    pub fn contains(&self, domain: &str) -> bool {
        self.lookup(domain).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suffix_match() {
        let mut trie = DomainTrie::new();
        trie.insert_suffix("google.com", "Proxy".to_string());

        assert_eq!(trie.lookup("google.com"), Some("Proxy"));
        assert_eq!(trie.lookup("www.google.com"), Some("Proxy"));
        assert_eq!(trie.lookup("mail.google.com"), Some("Proxy"));
        assert_eq!(trie.lookup("a.b.google.com"), Some("Proxy"));
        assert_eq!(trie.lookup("notgoogle.com"), None);
        assert_eq!(trie.lookup("baidu.com"), None);
    }

    #[test]
    fn test_multiple_rules() {
        let mut trie = DomainTrie::new();
        trie.insert_suffix("google.com", "Proxy".to_string());
        trie.insert_suffix("cn", "Direct".to_string());
        trie.insert_suffix("baidu.com", "Direct".to_string());

        assert_eq!(trie.lookup("www.google.com"), Some("Proxy"));
        assert_eq!(trie.lookup("www.baidu.com"), Some("Direct"));
        assert_eq!(trie.lookup("test.cn"), Some("Direct"));
    }
}
