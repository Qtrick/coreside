use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::models::WebSearchResponse;

const MAX_ENTRIES: usize = 64;
const TTL: Duration = Duration::from_secs(300);

struct CacheEntry {
    value: WebSearchResponse,
    expires: Instant,
}

/// Optional bounded in-memory cache for web search (tests / dedup within session).
pub struct SearchCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
}

impl Default for SearchCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl SearchCache {
    pub fn get(&self, key: &str) -> Option<WebSearchResponse> {
        let mut guard = self.inner.lock().ok()?;
        if let Some(entry) = guard.get(key) {
            if entry.expires > Instant::now() {
                return Some(entry.value.clone());
            }
            guard.remove(key);
        }
        None
    }

    pub fn put(&self, key: String, value: WebSearchResponse) {
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        if guard.len() >= MAX_ENTRIES {
            guard.retain(|_, e| e.expires > Instant::now());
            if guard.len() >= MAX_ENTRIES {
                if let Some(oldest) = guard.keys().next().cloned() {
                    guard.remove(&oldest);
                }
            }
        }
        guard.insert(
            key,
            CacheEntry {
                value,
                expires: Instant::now() + TTL,
            },
        );
    }
}
