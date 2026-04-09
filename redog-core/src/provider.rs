use async_trait::async_trait;
use std::sync::Arc;

use crate::adapter::ProxyAdapter;
use crate::error::Error;
use crate::metadata::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleType {
    File,
    Http,
    Compatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Proxy,
    Rule,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn vehicle_type(&self) -> VehicleType;
    fn provider_type(&self) -> ProviderType;
    async fn initialize(&self) -> Result<(), Error>;
    async fn update(&self) -> Result<(), Error>;
}

#[async_trait]
pub trait ProxyProvider: Provider {
    fn proxies(&self) -> Vec<Arc<dyn ProxyAdapter>>;
    async fn health_check(&self);
}

#[async_trait]
pub trait RuleProvider: Provider {
    fn matches(&self, metadata: &Metadata) -> bool;
    fn should_resolve_ip(&self) -> bool;
}
