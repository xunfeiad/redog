use arc_swap::ArcSwap;
use redog_adapter::outbound::direct::Direct;
use redog_core::adapter::ProxyAdapter;
use redog_core::conn::{ProxyStream, TrackedStream};
use redog_core::dns::DnsResolver;
use redog_core::error::Error;
use redog_core::metadata::Metadata;
use redog_core::rule::Rule;
use redog_listener::InboundConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::copy_bidirectional;

use crate::statistics::{ConnectionInfo, ConnectionTracker, TrafficCounter};

/// Tunnel mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Rule,
    Global,
    Direct,
}

/// Result of a rule match — uses Arc<str> to avoid repeated cloning
struct MatchResult {
    rule_name: Arc<str>,
    rule_payload: Arc<str>,
    adapter: Arc<dyn ProxyAdapter>,
}

/// The central routing engine
pub struct Tunnel {
    /// All available proxies (name -> adapter)
    proxies: ArcSwap<HashMap<String, Arc<dyn ProxyAdapter>>>,
    /// Ordered rule list
    rules: ArcSwap<Vec<Box<dyn Rule>>>,
    /// DNS resolver
    resolver: Arc<dyn DnsResolver>,
    /// Current tunnel mode
    mode: ArcSwap<Mode>,
    /// Traffic statistics
    pub traffic: Arc<TrafficCounter>,
    /// Connection tracker
    pub connections: Arc<ConnectionTracker>,
}

impl Tunnel {
    pub fn new(
        proxies: HashMap<String, Arc<dyn ProxyAdapter>>,
        rules: Vec<Box<dyn Rule>>,
        resolver: Arc<dyn DnsResolver>,
    ) -> Self {
        Self {
            proxies: ArcSwap::new(Arc::new(proxies)),
            rules: ArcSwap::new(Arc::new(rules)),
            resolver,
            mode: ArcSwap::new(Arc::new(Mode::Rule)),
            traffic: Arc::new(TrafficCounter::new()),
            connections: Arc::new(ConnectionTracker::new()),
        }
    }

    /// Handle an inbound TCP connection
    pub async fn handle_tcp(&self, conn: InboundConnection) {
        let metadata = conn.metadata;
        let client_stream = conn.stream;

        tracing::debug!("new connection: {}", metadata);

        // 1. Match rules to find the target adapter
        let m = self.match_adapter(&metadata).await;

        let adapter_name = m.adapter.name().to_string();
        tracing::info!(
            "{} -> {} [{}] ({})",
            metadata.destination(),
            adapter_name,
            m.rule_name,
            m.rule_payload
        );

        // 2. Connect to remote through the adapter
        let remote_stream = match m.adapter.connect_stream(&metadata).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "failed to connect {} via {}: {}",
                    metadata.destination(),
                    adapter_name,
                    e
                );
                return;
            }
        };

        // 3. Track connection
        let conn_info = ConnectionInfo {
            id: 0,
            destination: metadata.destination(),
            network: metadata.network.to_string(),
            inbound_type: metadata.inbound_type.to_string(),
            chains: vec![adapter_name.clone()],
            rule: m.rule_name.to_string(),
            rule_payload: m.rule_payload.to_string(),
            upload: 0,
            download: 0,
            start: chrono::Utc::now(),
        };
        let conn_id = self.connections.track(conn_info);

        // 4. Relay traffic
        let traffic = self.traffic.clone();
        let connections = self.connections.clone();

        // Convert TcpStream to a ProxyStream
        let mut client: Box<dyn ProxyStream> =
            Box::new(TrackedStream::new(client_stream));

        let mut remote = remote_stream;
        let result = copy_bidirectional(&mut client, &mut remote).await;

        match result {
            Ok((tx, rx)) => {
                traffic.add_upload(tx);
                traffic.add_download(rx);
                tracing::debug!(
                    "{} closed: tx={}, rx={}",
                    metadata.destination(),
                    tx,
                    rx
                );
            }
            Err(e) => {
                tracing::debug!("{} relay error: {}", metadata.destination(), e);
            }
        }

        connections.untrack(conn_id);
    }

    /// Match metadata against rules and return the adapter
    async fn match_adapter(&self, metadata: &Metadata) -> MatchResult {
        let mode = **self.mode.load();

        match mode {
            Mode::Direct => {
                let proxies = self.proxies.load();
                let adapter = proxies
                    .get("DIRECT")
                    .cloned()
                    .unwrap_or_else(|| Arc::new(Direct::new()));
                MatchResult {
                    rule_name: "DIRECT".into(),
                    rule_payload: "".into(),
                    adapter,
                }
            }
            Mode::Global => {
                let proxies = self.proxies.load();
                // In Global mode, use the first non-builtin proxy
                let adapter = proxies
                    .values()
                    .find(|p| {
                        p.name() != "DIRECT" && p.name() != "REJECT"
                    })
                    .cloned()
                    .unwrap_or_else(|| Arc::new(Direct::new()));
                MatchResult {
                    rule_name: "GLOBAL".into(),
                    rule_payload: "".into(),
                    adapter,
                }
            }
            Mode::Rule => self.rule_match(metadata).await,
        }
    }

    /// Match metadata against the ordered rule list
    async fn rule_match(&self, metadata: &Metadata) -> MatchResult {
        let rules = self.rules.load();
        let proxies = self.proxies.load();

        for rule in rules.iter() {
            if rule.matches(metadata) {
                let adapter_name = rule.adapter();
                if let Some(adapter) = proxies.get(adapter_name) {
                    return MatchResult {
                        rule_name: Arc::from(rule.rule_type().to_string()),
                        rule_payload: Arc::from(rule.payload()),
                        adapter: adapter.clone(),
                    };
                }
            }
        }

        // Default: DIRECT
        let adapter = proxies
            .get("DIRECT")
            .cloned()
            .unwrap_or_else(|| Arc::new(Direct::new()));
        MatchResult {
            rule_name: "MATCH".into(),
            rule_payload: "".into(),
            adapter,
        }
    }

    /// Update the proxy map
    pub fn update_proxies(&self, proxies: HashMap<String, Arc<dyn ProxyAdapter>>) {
        self.proxies.store(Arc::new(proxies));
    }

    /// Update the rule list
    pub fn update_rules(&self, rules: Vec<Box<dyn Rule>>) {
        self.rules.store(Arc::new(rules));
    }

    /// Set tunnel mode
    pub fn set_mode(&self, mode: Mode) {
        self.mode.store(Arc::new(mode));
        tracing::info!("tunnel mode changed to {:?}", mode);
    }

    /// Get current mode
    pub fn mode(&self) -> Mode {
        **self.mode.load()
    }

    /// Get proxy names
    pub fn proxy_names(&self) -> Vec<String> {
        self.proxies.load().keys().cloned().collect()
    }

    /// Get a proxy by name
    pub fn get_proxy(&self, name: &str) -> Option<Arc<dyn ProxyAdapter>> {
        self.proxies.load().get(name).cloned()
    }

    /// Get all rules info
    pub fn rules_info(&self) -> Vec<(String, String, String)> {
        self.rules
            .load()
            .iter()
            .map(|r| {
                (
                    r.rule_type().to_string(),
                    r.payload().to_string(),
                    r.adapter().to_string(),
                )
            })
            .collect()
    }
}
