use lru::LruCache as InnerLru;
use parking_lot::Mutex;
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroUsize;

/// Thread-safe LRU cache using parking_lot::Mutex for better performance
/// (no poisoning, smaller footprint, spin-first strategy)
pub struct LruCache<K: Hash + Eq, V> {
    inner: Mutex<InnerLru<K, V>>,
}

impl<K: Hash + Eq, V> fmt::Debug for LruCache<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.inner.lock().len();
        f.debug_struct("LruCache").field("len", &len).finish()
    }
}

impl<K: Hash + Eq, V: Clone> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or_else(|| {
            tracing::warn!("LruCache created with zero capacity, using 64");
            NonZeroUsize::new(64).unwrap()
        });
        Self {
            inner: Mutex::new(InnerLru::new(cap)),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().get(key).cloned()
    }

    pub fn put(&self, key: K, value: V) {
        self.inner.lock().put(key, value);
    }

    pub fn contains(&self, key: &K) -> bool {
        self.inner.lock().contains(key)
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.lock().pop(key)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }
}
