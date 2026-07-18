//! Lightweight ranking helpers for discovery candidates.

use serde_json::Value;

use crate::search::{normalize_web_results, validate_public_http_url, WebSearchResult};

/// Convert sidecar discovery candidates into ranked web search results.
pub fn rank_web_candidates(candidates: &[Value], query: &str, limit: usize) -> Vec<WebSearchResult> {
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().filter(|t| t.len() > 2).collect();

    let mut scored: Vec<(f64, WebSearchResult)> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let url_raw = c
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if url_raw.is_empty() {
            continue;
        }
        let Ok(safe_url) = validate_public_http_url(&url_raw) else {
            continue;
        };
        let url = safe_url.to_string();
        let title = c
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let snippet = c
            .get("snippet")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let base = c
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let hay = format!(
            "{} {} {}",
            title.to_lowercase(),
            snippet.as_deref().unwrap_or("").to_lowercase(),
            url.to_lowercase()
        );
        let overlap = terms.iter().filter(|t| hay.contains(*t)).count() as f64;
        let score = base + overlap * 0.5 - (i as f64) * 0.001;
        let domain = safe_url.host_str().map(|h| h.to_string());
        scored.push((
            score,
            WebSearchResult {
                id: format!("crawl-{}", i + 1),
                title: if title.is_empty() {
                    domain.clone().unwrap_or_else(|| url.clone())
                } else {
                    title
                },
                url,
                display_domain: domain,
                snippet,
                age: None,
                rank: 0,
            },
        ));
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let results: Vec<_> = scored.into_iter().take(limit.max(1)).map(|(_, r)| r).collect();
    normalize_web_results(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ranks_query_overlap_higher() {
        // Use public IP literals so unit tests do not depend on DNS.
        let cands = vec![
            json!({"url":"https://1.1.1.1/other","title":"Other","snippet":"","score":0.9}),
            json!({"url":"https://1.1.1.1/rust","title":"Rust guide","snippet":"learn rust","score":0.4}),
        ];
        let ranked = rank_web_candidates(&cands, "rust guide", 5);
        assert_eq!(ranked[0].url, "https://1.1.1.1/rust");
        assert_eq!(ranked[0].rank, 1);
    }

    #[test]
    fn drops_private_candidate_urls() {
        let cands = vec![
            json!({"url":"http://127.0.0.1/secret","title":"Nope","snippet":"","score":9.0}),
            json!({"url":"https://1.1.1.1/","title":"Public","snippet":"","score":0.1}),
        ];
        let ranked = rank_web_candidates(&cands, "public", 5);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].url, "https://1.1.1.1/");
    }
}
