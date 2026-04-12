use anyhow::Result;
use redog_adapter::outbound::direct::Direct;
use redog_adapter::outbound::reject::Reject;
use redog_core::adapter::ProxyAdapter;
use redog_core::rule::Rule;
use redog_dns::SystemResolver;
use redog_listener::InboundConnection;
use redog_tunnel::Tunnel;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Maximum concurrent connection handlers
const MAX_CONCURRENT_CONNECTIONS: usize = 4096;

/// Maximum retry attempts for listener restart
const MAX_LISTENER_RETRIES: u32 = 10;

/// Base delay between listener restarts
const LISTENER_RETRY_BASE_DELAY_MS: u64 = 500;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("redog v{}", env!("CARGO_PKG_VERSION"));

    // Load config
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());

    let config = match redog_config::load_config(&config_path) {
        Ok(c) => {
            tracing::info!("config loaded from {}", config_path);
            c
        }
        Err(e) => {
            tracing::warn!("failed to load config ({}), using defaults: {}", config_path, e);
            default_config()
        }
    };

    // Build proxies
    let mut proxies: HashMap<String, Arc<dyn ProxyAdapter>> = HashMap::new();
    proxies.insert("DIRECT".to_string(), Arc::new(Direct::new()));
    proxies.insert("REJECT".to_string(), Arc::new(Reject::new()));

    // Build rules
    let mut rules: Vec<Box<dyn Rule>> = Vec::new();
    for rule_str in &config.rules {
        match redog_rules::parse_rule(rule_str) {
            Ok(rule) => {
                tracing::debug!("rule: {} {} -> {}", rule.rule_type(), rule.payload(), rule.adapter());
                rules.push(rule);
            }
            Err(e) => {
                tracing::warn!("skip invalid rule '{}': {}", rule_str, e);
            }
        }
    }

    // Build DNS resolver
    let resolver = Arc::new(SystemResolver::new());

    // Create tunnel
    let tunnel = Arc::new(Tunnel::new(proxies, rules, resolver));

    // Channel for inbound connections
    let (tx, mut rx) = tokio::sync::mpsc::channel::<InboundConnection>(256);

    // Start listeners with auto-restart
    let mixed_port = config.mixed_port.unwrap_or(7890);
    let mixed_addr: SocketAddr = if config.allow_lan {
        format!("0.0.0.0:{}", mixed_port).parse()?
    } else {
        format!("127.0.0.1:{}", mixed_port).parse()?
    };

    // Mixed listener (HTTP + SOCKS5) with restart
    let tx_mixed = tx.clone();
    tokio::spawn(run_listener_with_restart("mixed", mixed_addr, move |addr| {
        let tx = tx_mixed.clone();
        async move { redog_listener::mixed::start_mixed_listener(addr, tx).await }
    }));

    // HTTP listener (if configured separately)
    if let Some(http_port) = config.port {
        let http_addr: SocketAddr = if config.allow_lan {
            format!("0.0.0.0:{}", http_port).parse()?
        } else {
            format!("127.0.0.1:{}", http_port).parse()?
        };
        let tx_http = tx.clone();
        tokio::spawn(run_listener_with_restart("HTTP", http_addr, move |addr| {
            let tx = tx_http.clone();
            async move { redog_listener::http::start_http_listener(addr, tx).await }
        }));
    }

    // SOCKS5 listener (if configured separately)
    if let Some(socks_port) = config.socks_port {
        let socks_addr: SocketAddr = if config.allow_lan {
            format!("0.0.0.0:{}", socks_port).parse()?
        } else {
            format!("127.0.0.1:{}", socks_port).parse()?
        };
        let tx_socks = tx.clone();
        tokio::spawn(run_listener_with_restart("SOCKS5", socks_addr, move |addr| {
            let tx = tx_socks.clone();
            async move { redog_listener::socks::start_socks_listener(addr, tx).await }
        }));
    }

    // Start API server
    if let Some(ref api_addr) = config.external_controller {
        let tunnel_api = tunnel.clone();
        let api_addr = api_addr.clone();
        let secret = config.secret.clone();
        tokio::spawn(async move {
            if let Err(e) = redog_api::start_api_server(&api_addr, tunnel_api, secret).await {
                tracing::error!("API server error: {}", e);
            }
        });
    }

    // Drop the original sender so channel closes when all listeners stop
    drop(tx);

    // Connection concurrency limiter
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

    // Main loop: receive inbound connections and dispatch to tunnel
    tracing::info!("tunnel started, waiting for connections...");
    while let Some(conn) = rx.recv().await {
        let tunnel = tunnel.clone();
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                tracing::error!("semaphore closed, shutting down");
                break;
            }
        };

        tokio::spawn(async move {
            tunnel.handle_tcp(conn).await;
            drop(permit); // release concurrency slot
        });
    }

    tracing::info!("all listeners stopped, shutting down");
    Ok(())
}

/// Run a listener with exponential backoff restart on failure
async fn run_listener_with_restart<F, Fut>(
    name: &'static str,
    addr: SocketAddr,
    make_listener: F,
) where
    F: Fn(SocketAddr) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), redog_core::error::Error>> + Send,
{
    let mut retries = 0u32;

    loop {
        match make_listener(addr).await {
            Ok(()) => {
                tracing::info!("{} listener on {} exited normally", name, addr);
                break;
            }
            Err(e) => {
                retries += 1;
                if retries > MAX_LISTENER_RETRIES {
                    tracing::error!(
                        "{} listener on {} failed {} times, giving up: {}",
                        name, addr, retries, e
                    );
                    break;
                }
                let delay = LISTENER_RETRY_BASE_DELAY_MS * 2u64.saturating_pow(retries - 1);
                tracing::warn!(
                    "{} listener error (attempt {}/{}), restarting in {}ms: {}",
                    name, retries, MAX_LISTENER_RETRIES, delay, e
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }
    }
}

fn default_config() -> redog_config::Config {
    redog_config::Config {
        port: None,
        socks_port: None,
        mixed_port: Some(7890),
        redir_port: None,
        tproxy_port: None,
        allow_lan: false,
        bind_address: "*".to_string(),
        mode: redog_config::TunnelMode::Rule,
        log_level: redog_config::LogLevel::Info,
        external_controller: Some("127.0.0.1:9090".to_string()),
        external_ui: None,
        secret: None,
        dns: None,
        tun: None,
        proxies: vec![],
        proxy_groups: vec![],
        proxy_providers: HashMap::new(),
        rule_providers: HashMap::new(),
        rules: vec!["MATCH,DIRECT".to_string()],
    }
}
