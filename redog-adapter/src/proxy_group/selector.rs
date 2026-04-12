use async_trait::async_trait;
use std::any::Any;
use std::sync::{Arc, RwLock};

use redog_core::adapter::{AdapterType, ProxyAdapter};
use redog_core::conn::{ProxyDatagram, ProxyStream};
use redog_core::error::Error;
use redog_core::metadata::Metadata;

/// Selector group — manual proxy selection via API
#[derive(Debug)]
pub struct Selector {
    name: String,
    proxies: Vec<Arc<dyn ProxyAdapter>>,
    selected: RwLock<usize>,
}

impl Selector {
    pub fn new(name: String, proxies: Vec<Arc<dyn ProxyAdapter>>) -> Self {
        Self {
            name,
            proxies,
            selected: RwLock::new(0),
        }
    }

    /// Switch the selected proxy by name
    pub fn select(&self, proxy_name: &str) -> Result<(), Error> {
        let idx = self
            .proxies
            .iter()
            .position(|p| p.name() == proxy_name)
            .ok_or_else(|| Error::Config(format!("proxy not found: {}", proxy_name)))?;
        *self.selected.write().unwrap() = idx;
        tracing::info!("Selector '{}': switched to '{}'", self.name, proxy_name);
        Ok(())
    }

    /// Get the currently selected proxy name
    pub fn current(&self) -> String {
        let idx = *self.selected.read().unwrap();
        self.proxies
            .get(idx)
            .map(|p| p.name().to_string())
            .unwrap_or_else(|| "DIRECT".to_string())
    }

    /// Get all proxy names
    pub fn all(&self) -> Vec<String> {
        self.proxies.iter().map(|p| p.name().to_string()).collect()
    }

    fn selected_proxy(&self) -> Arc<dyn ProxyAdapter> {
        let idx = *self.selected.read().unwrap();
        self.proxies
            .get(idx)
            .cloned()
            .unwrap_or_else(|| self.proxies.first().expect("selector must have proxies").clone())
    }
}

#[async_trait]
impl ProxyAdapter for Selector {
    fn name(&self) -> &str {
        &self.name
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Selector
    }

    async fn connect_stream(&self, metadata: &Metadata) -> Result<Box<dyn ProxyStream>, Error> {
        self.selected_proxy().connect_stream(metadata).await
    }

    async fn connect_datagram(
        &self,
        metadata: &Metadata,
    ) -> Result<Box<dyn ProxyDatagram>, Error> {
        self.selected_proxy().connect_datagram(metadata).await
    }

    fn support_udp(&self) -> bool {
        self.selected_proxy().support_udp()
    }

    fn alive(&self) -> bool {
        self.selected_proxy().alive()
    }

    fn unwrap_adapter(&self) -> Option<Arc<dyn ProxyAdapter>> {
        Some(self.selected_proxy())
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}
