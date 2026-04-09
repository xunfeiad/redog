use async_trait::async_trait;
use std::sync::{Arc, RwLock};
use tokio::time::Duration;

use redog_core::adapter::{AdapterType, ProxyAdapter};
use redog_core::conn::{ProxyDatagram, ProxyStream};
use redog_core::error::Error;
use redog_core::metadata::Metadata;

/// URLTest group — auto-select lowest latency proxy
#[derive(Debug)]
pub struct URLTest {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    test_url: String,
    interval: Duration,
    tolerance: u16,
    fastest: RwLock<usize>,
    delays: RwLock<Vec<u16>>,
}

impl URLTest {
    pub fn new(
        name: String,
        proxies: Vec<Arc<dyn ProxyAdapter>>,
        test_url: String,
        interval_secs: u64,
        tolerance: u16,
    ) -> Self {
        let len = proxies.len();
        Self {
            name,
            proxies,
            test_url,
            interval: Duration::from_secs(interval_secs),
            tolerance,
            fastest: RwLock::new(0),
            delays: RwLock::new(vec![u16::MAX; len]),
        }
    }

    /// Start the background health check loop
    pub fn start_health_check(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(self.interval);
            loop {
                ticker.tick().await;
                self.check_all().await;
            }
        });
    }

    async fn check_all(&self) {
        let mut results = Vec::with_capacity(self.proxies.len());
        for (idx, proxy) in self.proxies.iter().enumerate() {
            let delay = Self::test_delay(proxy.clone(), &self.test_url).await;
            results.push((idx, delay));
        }

        // Update delays
        {
            let mut delays = self.delays.write().unwrap();
            for (idx, delay) in &results {
                delays[*idx] = *delay;
            }
        }

        // Find fastest
        let current = *self.fastest.read().unwrap();
        let current_delay = results
            .iter()
            .find(|(i, _)| *i == current)
            .map(|(_, d)| *d)
            .unwrap_or(u16::MAX);

        if let Some((new_fastest, new_delay)) = results.iter().min_by_key(|(_, d)| *d) {
            if *new_delay + self.tolerance < current_delay {
                *self.fastest.write().unwrap() = *new_fastest;
                tracing::info!(
                    "URLTest '{}': switched to '{}' ({}ms -> {}ms)",
                    self.name,
                    self.proxies[*new_fastest].name(),
                    current_delay,
                    new_delay,
                );
            }
        }
    }

    async fn test_delay(proxy: Arc<dyn ProxyAdapter>, url: &str) -> u16 {
        let start = tokio::time::Instant::now();
        let metadata = Metadata::new(
            redog_core::metadata::Network::Tcp,
            redog_core::metadata::InboundType::Inner,
        );

        match tokio::time::timeout(Duration::from_secs(5), proxy.connect_stream(&metadata)).await {
            Ok(Ok(_)) => start.elapsed().as_millis() as u16,
            _ => u16::MAX,
        }
    }

    pub fn current(&self) -> String {
        let idx = *self.fastest.read().unwrap();
        self.proxies[idx].name().to_string()
    }

    pub fn all(&self) -> Vec<String> {
        self.proxies.iter().map(|p| p.name().to_string()).collect()
    }

    fn fastest_proxy(&self) -> Arc<dyn ProxyAdapter> {
        let idx = *self.fastest.read().unwrap();
        self.proxies[idx].clone()
    }
}

#[async_trait]
impl ProxyAdapter for URLTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::URLTest
    }

    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        self.fastest_proxy().connect_stream(metadata).await
    }

    async fn connect_datagram(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error> {
        self.fastest_proxy().connect_datagram(metadata).await
    }

    fn support_udp(&self) -> bool {
        self.fastest_proxy().support_udp()
    }

    fn alive(&self) -> bool {
        self.fastest_proxy().alive()
    }

    fn unwrap_adapter(&self) -> Option<Arc<dyn ProxyAdapter>> {
        Some(self.fastest_proxy())
    }
}
