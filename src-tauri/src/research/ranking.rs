//! Lightweight ranking helpers for discovery candidates.

use serde_json::Value;

use crate::search::{normalize_web_results, validate_public_http_url, WebSearchResult};

/// Convert sidecar discovery candidates into ranked web search results.
pub fn rank_web_candidates(
    candidates: &[Value],
    query: &str,
    limit: usize,
) -> Vec<WebSearchResult> {
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
        let base = c.get("score").and_then(|v| v.as_f64()).unwrap_or(0.5);
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
                score: Some(score),
                ..Default::default()
            },
        ));
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let results: Vec<_> = scored
        .into_iter()
        .take(limit.max(1))
        .map(|(_, r)| r)
        .collect();
    normalize_web_results(results)
}

/// Multi-signal ranking for normalized search results from one or more providers.
/// Balances lexical overlap, provider relevance, content richness, and upstream position.
pub fn rank_search_results(results: &mut Vec<WebSearchResult>, query: &str) {
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().filter(|t| t.len() > 2).collect();

    let mut scored: Vec<(f64, usize)> = results
        .iter()
        .enumerate()
        .map(|(orig_idx, r)| {
            let mut score = 1.0;

            // 1. Initial provider baseline
            match r.provider.as_deref() {
                Some("exa") => score += 0.4,
                Some("linkup") => score += 0.35,
                Some("firecrawl") => score += 0.3,
                Some("crawl4ai") => score += 0.25,
                _ => score += 0.2,
            }

            // 2. Lexical overlap
            let title_lower = r.title.to_lowercase();
            let snippet_lower = r.snippet.as_deref().unwrap_or("").to_lowercase();
            let url_lower = r.url.to_lowercase();

            for term in &terms {
                if title_lower.contains(*term) {
                    score += 0.8;
                }
                if snippet_lower.contains(*term) {
                    score += 0.4;
                }
                if url_lower.contains(*term) {
                    score += 0.3;
                }
            }

            // 3. Content richness bonus
            if r.content.is_some() {
                score += 0.5;
            }
            if r.highlights.as_ref().is_some_and(|h| !h.is_empty()) {
                score += 0.3;
            }

            // 4. Freshness bonus
            if r.age.is_some() || r.fetched_at.is_some() {
                score += 0.1;
            }

            // 5. Position tie-breaker
            score -= (orig_idx as f64) * 0.05;

            (score, orig_idx)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let orig = std::mem::take(results);
    let mut orig_map: std::collections::HashMap<usize, WebSearchResult> =
        orig.into_iter().enumerate().collect();

    for (new_rank, (final_score, orig_idx)) in scored.into_iter().enumerate() {
        if let Some(mut r) = orig_map.remove(&orig_idx) {
            r.rank = new_rank + 1;
            r.score = Some((final_score * 100.0).round() / 100.0);
            results.push(r);
        }
    }
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

    #[test]
    fn rank_search_results_prioritizes_relevance_and_richness() {
        let mut results = vec![
            WebSearchResult {
                id: "1".into(),
                title: "Irrelevant page".into(),
                url: "https://example.com/other".into(),
                display_domain: Some("example.com".into()),
                snippet: Some("no matches here".into()),
                rank: 1,
                provider: Some("linkup".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "2".into(),
                title: "Tauri v2 Desktop Security Architecture".into(),
                url: "https://example.com/tauri-security".into(),
                display_domain: Some("example.com".into()),
                snippet: Some("Overview of security architecture in Tauri v2 desktop apps".into()),
                content: Some("# Full Security Architecture Details".into()),
                rank: 2,
                provider: Some("exa".into()),
                ..Default::default()
            },
        ];

        rank_search_results(&mut results, "tauri security desktop");
        assert_eq!(results[0].id, "2");
        assert_eq!(results[0].rank, 1);
        assert!(results[0].score.unwrap() > results[1].score.unwrap());
    }
}
