//! Short-TTL in-memory cache for Exa search responses.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::models::ExaSearchResponse;
use super::profiles::SearchProfile;

const MAX_ENTRIES: usize = 64;
const TTL: Duration = Duration::from_secs(300);

struct CacheEntry {
    value: ExaSearchResponse,
    expires: Instant,
}

pub struct ExaSearchCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
}

impl Default for ExaSearchCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl ExaSearchCache {
    pub fn get(&self, key: &str) -> Option<ExaSearchResponse> {
        let mut guard = self.inner.lock().ok()?;
        if let Some(entry) = guard.get(key) {
            if entry.expires > Instant::now() {
                return Some(entry.value.clone());
            }
            guard.remove(key);
        }
        None
    }

    pub fn put(&self, key: String, value: ExaSearchResponse) {
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

/// Normalize query text for stable cache fingerprints.
pub fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn fingerprint(
    query: &str,
    profile: SearchProfile,
    include_domains: Option<&[String]>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize_query(query).as_bytes());
    hasher.update(b"|");
    hasher.update(profile.search_type().as_bytes());
    hasher.update(b"|");
    hasher.update(profile.num_results().to_string().as_bytes());
    if let Some(domains) = include_domains {
        let mut sorted = domains.to_vec();
        sorted.sort();
        for d in sorted {
            hasher.update(b"|");
            hasher.update(d.to_lowercase().as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace_and_case() {
        assert_eq!(normalize_query("  Hello   WORLD  "), "hello world");
        assert_eq!(normalize_query("a\tb\nc"), "a b c");
    }

    #[test]
    fn fingerprint_stable_for_equivalent_queries() {
        let a = fingerprint("Rust async", SearchProfile::Saver, None);
        let b = fingerprint("  rust   ASYNC ", SearchProfile::Saver, None);
        assert_eq!(a, b);
        let c = fingerprint("Rust async", SearchProfile::Balanced, None);
        assert_ne!(a, c);
    }

    #[test]
    fn fingerprint_includes_domains() {
        let a = fingerprint(
            "topic",
            SearchProfile::Saver,
            Some(&["b.com".into(), "a.com".into()]),
        );
        let b = fingerprint(
            "topic",
            SearchProfile::Saver,
            Some(&["a.com".into(), "b.com".into()]),
        );
        assert_eq!(a, b);
    }
}
