use lru::LruCache as InnerLru;
use std::fmt;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// Thread-safe LRU cache
pub struct LruCache<K: Hash + Eq, V> {
    inner: Mutex<InnerLru<K, V>>,
}

impl<K: Hash + Eq, V> fmt::Debug for LruCache<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.inner.lock().unwrap().len();
        f.debug_struct("LruCache").field("len", &len).finish()
    }
}

impl<K: Hash + Eq, V: Clone> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(InnerLru::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap()),
            )),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().unwrap().get(key).cloned()
    }

    pub fn put(&self, key: K, value: V) {
        self.inner.lock().unwrap().put(key, value);
    }

    pub fn contains(&self, key: &K) -> bool {
        self.inner.lock().unwrap().contains(key)
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.lock().unwrap().pop(key)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }
}
