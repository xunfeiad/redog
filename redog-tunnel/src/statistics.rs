use dashmap::DashMap;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global traffic counter
pub struct TrafficCounter {
    upload_total: AtomicU64,
    download_total: AtomicU64,
    last_upload: AtomicU64,
    last_download: AtomicU64,
}

impl TrafficCounter {
    pub fn new() -> Self {
        Self {
            upload_total: AtomicU64::new(0),
            download_total: AtomicU64::new(0),
            last_upload: AtomicU64::new(0),
            last_download: AtomicU64::new(0),
        }
    }

    pub fn add_upload(&self, bytes: u64) {
        self.upload_total.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_download(&self, bytes: u64) {
        self.download_total.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TrafficSnapshot {
        let upload = self.upload_total.load(Ordering::Relaxed);
        let download = self.download_total.load(Ordering::Relaxed);
        let last_up = self.last_upload.swap(upload, Ordering::Relaxed);
        let last_down = self.last_download.swap(download, Ordering::Relaxed);

        TrafficSnapshot {
            upload_speed: upload.saturating_sub(last_up),
            download_speed: download.saturating_sub(last_down),
            upload_total: upload,
            download_total: download,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TrafficSnapshot {
    pub upload_speed: u64,
    pub download_speed: u64,
    pub upload_total: u64,
    pub download_total: u64,
}

/// Connection tracker
pub struct ConnectionTracker {
    connections: DashMap<u64, ConnectionInfo>,
    counter: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub id: u64,
    pub destination: String,
    pub network: String,
    pub inbound_type: String,
    pub chains: Vec<String>,
    pub rule: String,
    pub rule_payload: String,
    pub upload: u64,
    pub download: u64,
    pub start: chrono::DateTime<chrono::Utc>,
}

impl ConnectionTracker {
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            counter: AtomicU64::new(0),
        }
    }

    pub fn track(&self, info: ConnectionInfo) -> u64 {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        self.connections.insert(id, info);
        id
    }

    pub fn untrack(&self, id: u64) {
        self.connections.remove(&id);
    }

    pub fn list(&self) -> Vec<ConnectionInfo> {
        self.connections
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn close_all(&self) -> usize {
        let count = self.connections.len();
        self.connections.clear();
        count
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}
