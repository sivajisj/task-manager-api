use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CacheEntry {
    value: Value,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

#[derive(Clone, Debug)]
pub struct AppCache {
    store: Arc<DashMap<String, CacheEntry>>,
}

impl AppCache {
    pub fn new() -> Self {
        AppCache {
            store: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        if let Some(entry) = self.store.get(key) {
            if entry.is_expired() {
                drop(entry);
                self.store.remove(key);
                return None;
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub fn set(&self, key: String, value: Value, ttl_seconds: u64) {
        let entry = CacheEntry {
            value,
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(ttl_seconds),
        };
        self.store.insert(key, entry);
    }

    pub fn invalidate(&self, key: &str) {
        self.store.remove(key);
    }

    pub fn invalidate_prefix(&self, prefix: &str) {
        let keys_to_remove: Vec<String> = self
            .store
            .iter()
            .filter(|e| e.key().starts_with(prefix))
            .map(|e| e.key().clone())
            .collect();
        for key in keys_to_remove {
            self.store.remove(&key);
        }
    }
}

impl Default for AppCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn task_cache_key(user_id: &str) -> String {
    format!("tasks:user:{}", user_id)
}