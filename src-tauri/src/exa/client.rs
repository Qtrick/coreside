//! Direct HTTP Exa Search client (no Agent API, no SDK).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use reqwest::Client;
use tokio::sync::broadcast;
use tokio::time::{sleep, timeout};

use super::cache::{fingerprint, ExaSearchCache};
use super::credentials::require_exa_api_key;
use super::errors::ExaError;
use super::models::{estimate_search_cost, ExaSearchRequest, ExaSearchResponse};
use super::profiles::SearchProfile;

const EXA_SEARCH_URL: &str = "https://api.exa.ai/search";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 2;

fn http_client() -> Result<&'static Client, ExaError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let built = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Coreside/0.1 (+https://coreside.local)")
        .build()
        .map_err(|e| ExaError::Other(e.to_string()))?;
    let _ = CLIENT.set(built);
    CLIENT
        .get()
        .ok_or_else(|| ExaError::Other("http client init failed".into()))
}

fn shared_cache() -> &'static ExaSearchCache {
    static CACHE: OnceLock<ExaSearchCache> = OnceLock::new();
    CACHE.get_or_init(ExaSearchCache::default)
}

type InflightMap = Mutex<HashMap<String, broadcast::Sender<Result<ExaSearchResponse, ExaError>>>>;

fn inflight() -> &'static InflightMap {
    static MAP: OnceLock<InflightMap> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Removes this leader's in-flight entry on any exit (success, error, or cancel).
struct InflightLead {
    key: String,
    tx: broadcast::Sender<Result<ExaSearchResponse, ExaError>>,
}

impl Drop for InflightLead {
    fn drop(&mut self) {
        if let Ok(mut guard) = inflight().lock() {
            if guard
                .get(&self.key)
                .is_some_and(|existing| existing.same_channel(&self.tx))
            {
                guard.remove(&self.key);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExaSearchOutcome {
    pub response: ExaSearchResponse,
    /// True for memory-cache hits and coalesced followers (no extra Exa spend).
    pub cache_hit: bool,
    pub search_mode: String,
    pub estimated_cost: f64,
}

fn cache_hit_outcome(profile_mode: &str, response: ExaSearchResponse) -> ExaSearchOutcome {
    ExaSearchOutcome {
        // Cache hits / coalesced followers are free — do not attribute estimated spend.
        estimated_cost: 0.0,
        search_mode: profile_mode.to_string(),
        response,
        cache_hit: true,
    }
}

/// Return a cached Exa response without requiring credentials or budget.
pub fn peek_cached_search(
    query: &str,
    profile: SearchProfile,
    include_domains: Option<&[String]>,
) -> Option<ExaSearchOutcome> {
    let cache_key = fingerprint(query, profile, include_domains);
    shared_cache()
        .get(&cache_key)
        .map(|cached| cache_hit_outcome(profile.search_type(), cached))
}

/// Run Exa Search with highlights. Never calls Exa Agent. Never defaults to deep-reasoning.
/// Identical in-flight queries coalesce to one HTTP request.
pub async fn search(
    query: &str,
    profile: SearchProfile,
    include_domains: Option<Vec<String>>,
) -> Result<ExaSearchOutcome, ExaError> {
    let domains_ref = include_domains.as_deref();
    if let Some(cached) = peek_cached_search(query, profile, domains_ref) {
        return Ok(cached);
    }

    let (api_key, _) = require_exa_api_key()?;
    let mut request = ExaSearchRequest::for_profile(query, profile);
    if let Some(domains) = include_domains {
        request = request.with_include_domains(domains);
    }
    // Hard-cap results ≤10.
    request.num_results = request.num_results.min(10);
    let cache_key = fingerprint(query, profile, request.include_domains.as_deref());
    let search_mode = request.search_type.clone();

    // Join an identical in-flight request when present; otherwise lead one HTTP call.
    let lead = loop {
        // Another leader may have finished and filled the cache while we waited.
        if let Some(cached) = shared_cache().get(&cache_key) {
            return Ok(cache_hit_outcome(&search_mode, cached));
        }

        let follower = {
            let guard = inflight()
                .lock()
                .map_err(|_| ExaError::Other("inflight lock".into()))?;
            guard.get(&cache_key).map(|tx| tx.subscribe())
        };
        if let Some(mut rx) = follower {
            match rx.recv().await {
                Ok(Ok(response)) => {
                    return Ok(cache_hit_outcome(&search_mode, response));
                }
                Ok(Err(e)) => return Err(e),
                Err(broadcast::error::RecvError::Closed) => {
                    // Leader exited without delivering (cancel/panic) or we subscribed
                    // after the single send — prefer cache, else retry leadership.
                    if let Some(cached) = shared_cache().get(&cache_key) {
                        return Ok(cache_hit_outcome(&search_mode, cached));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Missed the coalesced value; cache should hold it when the leader finished.
                    if let Some(cached) = shared_cache().get(&cache_key) {
                        return Ok(cache_hit_outcome(&search_mode, cached));
                    }
                }
            }
        }

        let (tx, _rx) = broadcast::channel::<Result<ExaSearchResponse, ExaError>>(8);
        let mut guard = inflight()
            .lock()
            .map_err(|_| ExaError::Other("inflight lock".into()))?;
        // Re-check cache under the inflight lock to close the finish→join TOCTOU.
        if let Some(cached) = shared_cache().get(&cache_key) {
            return Ok(cache_hit_outcome(&search_mode, cached));
        }
        if guard.contains_key(&cache_key) {
            // Lost the race — loop and subscribe.
            continue;
        }
        guard.insert(cache_key.clone(), tx.clone());
        break InflightLead {
            key: cache_key.clone(),
            tx,
        };
    };

    // Final cache check after winning leadership (another request may have just finished).
    if let Some(cached) = shared_cache().get(&cache_key) {
        let _ = lead.tx.send(Ok(cached.clone()));
        return Ok(cache_hit_outcome(&search_mode, cached));
    }

    let result = search_http(&api_key, &request).await;
    match &result {
        Ok(response) => {
            // Populate cache before notifying followers so late joiners can fall back to it.
            shared_cache().put(cache_key.clone(), response.clone());
            let _ = lead.tx.send(Ok(response.clone()));
        }
        Err(e) => {
            let _ = lead.tx.send(Err(e.clone()));
        }
    }
    // InflightLead Drop removes the map entry (also on cancel/panic).

    let response = result?;
    Ok(ExaSearchOutcome {
        estimated_cost: estimate_search_cost(&request.search_type, request.num_results),
        search_mode,
        response,
        cache_hit: false,
    })
}

/// Lightweight connectivity check (1 result, fast) using the currently resolved key.
pub async fn test_connection() -> Result<String, ExaError> {
    let (api_key, source) = require_exa_api_key()?;
    test_connection_with_key(&api_key).await?;
    Ok(format!(
        "Exa Search OK (credential source: {})",
        source.as_str()
    ))
}

/// Probe Exa with an explicit key without requiring keyring persistence first.
pub async fn test_connection_with_key(api_key: &str) -> Result<(), ExaError> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err(ExaError::NotConfigured("Exa API key is required".into()));
    }
    let request = ExaSearchRequest {
        query: "coreside connectivity probe".into(),
        search_type: "fast".into(),
        num_results: 1,
        include_domains: None,
        contents: super::models::ExaContentsRequest { highlights: true },
    };
    let _ = search_http(key, &request).await?;
    Ok(())
}

async fn search_http(
    api_key: &str,
    request: &ExaSearchRequest,
) -> Result<ExaSearchResponse, ExaError> {
    let client = http_client()?;
    let mut attempt = 0u32;
    loop {
        let fut = client
            .post(EXA_SEARCH_URL)
            .header("x-api-key", api_key)
            .header("content-type", "application/json")
            .json(request)
            .send();

        let result = timeout(REQUEST_TIMEOUT, fut).await;
        match result {
            Err(_) => return Err(ExaError::Timeout),
            Ok(Err(e)) => {
                if attempt < MAX_RETRIES && e.is_timeout() {
                    attempt += 1;
                    sleep(jitter(attempt)).await;
                    continue;
                }
                return Err(ExaError::ProviderUnavailable(e.to_string()));
            }
            Ok(Ok(resp)) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                if status == 429 && attempt < MAX_RETRIES {
                    attempt += 1;
                    sleep(jitter(attempt)).await;
                    continue;
                }
                if !(200..300).contains(&status) {
                    // Never retry 401/402/403/422.
                    return Err(ExaError::from_status(status, &body));
                }
                return serde_json::from_str(&body)
                    .map_err(|e| ExaError::Other(format!("failed to parse Exa response: {e}")));
            }
        }
    }
}

fn jitter(attempt: u32) -> Duration {
    let base_ms = 200u64 * u64::from(attempt).max(1);
    Duration::from_millis(base_ms + (attempt as u64 * 37) % 80)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_body_uses_highlights_not_full_text() {
        let req = ExaSearchRequest::for_profile("rust", SearchProfile::Saver);
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["type"], "fast");
        assert_eq!(v["numResults"], 3);
        assert_eq!(v["contents"]["highlights"], true);
        assert!(v["contents"].get("text").is_none());
    }

    #[test]
    fn parses_live_shaped_payload() {
        let raw = json!({
            "requestId": "abc",
            "costDollars": { "total": 0.007 },
            "results": [{
                "title": "Rust Book",
                "url": "https://doc.rust-lang.org/book/",
                "highlights": ["async runtime"]
            }]
        });
        let parsed: ExaSearchResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.actual_cost(), Some(0.007));
    }
}
