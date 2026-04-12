use redog_core::metadata::DnsMode;
use redog_core::Error;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelMode {
    Rule,
    Global,
    Direct,
}

impl Default for TunnelMode {
    fn default() -> Self {
        TunnelMode::Rule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Silent,
    Error,
    Warning,
    Info,
    Debug,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub port: Option<u16>,

    #[serde(rename = "socks-port", default)]
    pub socks_port: Option<u16>,

    #[serde(rename = "mixed-port", default)]
    pub mixed_port: Option<u16>,

    #[serde(rename = "redir-port", default)]
    pub redir_port: Option<u16>,

    #[serde(rename = "tproxy-port", default)]
    pub tproxy_port: Option<u16>,

    #[serde(rename = "allow-lan", default)]
    pub allow_lan: bool,

    #[serde(rename = "bind-address", default = "default_bind_address")]
    pub bind_address: String,

    #[serde(default)]
    pub mode: TunnelMode,

    #[serde(rename = "log-level", default)]
    pub log_level: LogLevel,

    #[serde(rename = "external-controller", default)]
    pub external_controller: Option<String>,

    #[serde(rename = "external-ui", default)]
    pub external_ui: Option<String>,

    #[serde(default)]
    pub secret: Option<String>,

    #[serde(default)]
    pub dns: Option<DnsConfig>,

    #[serde(default)]
    pub tun: Option<TunConfig>,

    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,

    #[serde(rename = "proxy-groups", default)]
    pub proxy_groups: Vec<ProxyGroupConfig>,

    #[serde(rename = "proxy-providers", default)]
    pub proxy_providers: HashMap<String, ProviderConfig>,

    #[serde(rename = "rule-providers", default)]
    pub rule_providers: HashMap<String, RuleProviderConfig>,

    #[serde(default)]
    pub rules: Vec<String>,
}

fn default_bind_address() -> String {
    "*".to_string()
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        // Validate port ranges (must be 1-65535)
        let all_ports: Vec<(&str, Option<u16>)> = vec![
            ("port", self.port),
            ("socks-port", self.socks_port),
            ("mixed-port", self.mixed_port),
            ("redir-port", self.redir_port),
            ("tproxy-port", self.tproxy_port),
        ];
        for (name, maybe_port) in &all_ports {
            if let Some(p) = maybe_port {
                if *p == 0 {
                    return Err(Error::Config(format!(
                        "invalid {} value: port must be 1-65535",
                        name
                    )));
                }
            }
        }

        // Check for port conflicts
        let mut ports = Vec::new();
        for (name, maybe_port) in &all_ports {
            if let Some(p) = maybe_port {
                if ports.contains(p) {
                    return Err(Error::Config(format!(
                        "port conflict: {} ({}) is already in use",
                        name, p
                    )));
                }
                ports.push(*p);
            }
        }

        // Validate proxy group references
        let proxy_names: HashSet<String> = self
            .proxies
            .iter()
            .map(|p| p.name().to_string())
            .chain(self.proxy_groups.iter().map(|g| g.name.clone()))
            .chain(
                ["DIRECT", "REJECT", "GLOBAL"]
                    .iter()
                    .map(|s| s.to_string()),
            )
            .collect();

        for group in &self.proxy_groups {
            if group.proxies.is_empty() && group.use_providers.is_empty() {
                return Err(Error::Config(format!(
                    "proxy group '{}' has no proxies or providers",
                    group.name
                )));
            }
            for proxy_name in &group.proxies {
                if !proxy_names.contains(proxy_name) {
                    return Err(Error::Config(format!(
                        "proxy group '{}' references unknown proxy: {}",
                        group.name, proxy_name
                    )));
                }
            }
        }

        // Validate rule syntax (basic check)
        for (i, rule_str) in self.rules.iter().enumerate() {
            let parts: Vec<&str> = rule_str.splitn(3, ',').collect();
            if parts.len() < 2 {
                return Err(Error::Config(format!(
                    "rule #{} '{}': must have at least type and target",
                    i + 1,
                    rule_str
                )));
            }
        }

        // Validate DNS config
        if let Some(ref dns) = self.dns {
            if dns.enable && dns.nameserver.is_empty() {
                return Err(Error::Config(
                    "DNS is enabled but no nameservers configured".into(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DnsConfig {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub listen: Option<String>,

    #[serde(rename = "enhanced-mode", default)]
    pub enhanced_mode: Option<DnsMode>,

    #[serde(rename = "fake-ip-range", default)]
    pub fake_ip_range: Option<String>,

    #[serde(rename = "fake-ip-filter", default)]
    pub fake_ip_filter: Vec<String>,

    #[serde(default)]
    pub nameserver: Vec<String>,

    #[serde(default)]
    pub fallback: Vec<String>,

    #[serde(rename = "nameserver-policy", default)]
    pub nameserver_policy: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunConfig {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_tun_stack")]
    pub stack: String,

    #[serde(rename = "dns-hijack", default)]
    pub dns_hijack: Vec<String>,

    #[serde(rename = "auto-route", default)]
    pub auto_route: bool,

    #[serde(rename = "auto-detect-interface", default)]
    pub auto_detect_interface: bool,
}

fn default_tun_stack() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub proxy_type: String,

    #[serde(default)]
    pub server: Option<String>,

    #[serde(default)]
    pub port: Option<u16>,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default)]
    pub cipher: Option<String>,

    #[serde(default)]
    pub uuid: Option<String>,

    #[serde(rename = "alterId", default)]
    pub alter_id: Option<u16>,

    #[serde(default)]
    pub tls: Option<bool>,

    #[serde(default)]
    pub sni: Option<String>,

    #[serde(default)]
    pub network: Option<String>,

    #[serde(rename = "ws-opts", default)]
    pub ws_opts: Option<WsOpts>,

    #[serde(default)]
    pub udp: Option<bool>,

    #[serde(rename = "skip-cert-verify", default)]
    pub skip_cert_verify: Option<bool>,
}

impl ProxyConfig {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsOpts {
    #[serde(default)]
    pub path: Option<String>,

    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyGroupConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub group_type: String,

    #[serde(default)]
    pub proxies: Vec<String>,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub interval: Option<u64>,

    #[serde(default)]
    pub tolerance: Option<u16>,

    #[serde(rename = "use", default)]
    pub use_providers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub vehicle_type: String,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub path: Option<String>,

    #[serde(default)]
    pub interval: Option<u64>,

    #[serde(rename = "health-check", default)]
    pub health_check: Option<HealthCheckConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthCheckConfig {
    #[serde(default)]
    pub enable: bool,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleProviderConfig {
    #[serde(rename = "type")]
    pub vehicle_type: String,

    #[serde(default)]
    pub behavior: Option<String>,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub path: Option<String>,

    #[serde(default)]
    pub interval: Option<u64>,
}
