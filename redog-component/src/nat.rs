use dashmap::DashMap;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

use redog_core::conn::ProxyDatagram;

/// UDP NAT session
pub struct UdpSession {
    pub datagram: Arc<dyn ProxyDatagram>,
    pub last_active: Instant,
}

/// NAT table for UDP session tracking
pub struct NatTable {
    table: DashMap<String, UdpSession>,
    timeout: Duration,
}

impl NatTable {
    pub fn new(timeout: Duration) -> Self {
        Self {
            table: DashMap::new(),
            timeout,
        }
    }

    /// Get an existing session
    pub fn get(&self, key: &str) -> Option<Arc<dyn ProxyDatagram>> {
        if let Some(mut session) = self.table.get_mut(key) {
            session.last_active = Instant::now();
            Some(session.datagram.clone())
        } else {
            None
        }
    }

    /// Insert a new session
    pub fn insert(&self, key: String, datagram: Arc<dyn ProxyDatagram>) {
        self.table.insert(
            key,
            UdpSession {
                datagram,
                last_active: Instant::now(),
            },
        );
    }

    /// Remove a session
    pub fn remove(&self, key: &str) {
        self.table.remove(key);
    }

    /// Clean up expired sessions
    pub fn cleanup(&self) -> usize {
        let before = self.table.len();
        self.table
            .retain(|_, session| session.last_active.elapsed() < self.timeout);
        before - self.table.len()
    }

    /// Start background cleanup loop
    pub fn start_cleanup_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let removed = self.cleanup();
                if removed > 0 {
                    tracing::debug!("NAT table cleanup: removed {} expired sessions", removed);
                }
            }
        });
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}
