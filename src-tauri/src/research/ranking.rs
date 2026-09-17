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

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "what", "how", "are", "this", "that", "from", "when", "where",
    "which", "who", "why", "can", "you", "does", "about", "into", "over", "after", "before",
    "between", "some", "then",
];

/// Multi-signal ranking for normalized search results from one or more providers.
/// Balances lexical overlap, phrase matching, provider relevance, content richness,
/// primary source authority, domain diversity, and upstream position.
pub fn rank_search_results(results: &mut Vec<WebSearchResult>, query: &str) {
    let q = query.trim().to_lowercase();
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|t| t.len() > 2 && !STOP_WORDS.contains(t))
        .map(String::from)
        .collect();

    let clean_query = q.trim();
    let has_phrase = clean_query.len() >= 4 && clean_query.contains(' ');

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

            // 2. Lexical overlap with stop-word normalized terms
            let title_lower = r.title.to_lowercase();
            let snippet_lower = r.snippet.as_deref().unwrap_or("").to_lowercase();
            let url_lower = r.url.to_lowercase();

            for term in &terms {
                if title_lower.contains(term.as_str()) {
                    score += 0.8;
                }
                if snippet_lower.contains(term.as_str()) {
                    score += 0.4;
                }
                if url_lower.contains(term.as_str()) {
                    score += 0.3;
                }
            }

            // 3. Exact phrase match bonus
            if has_phrase {
                if title_lower.contains(clean_query) {
                    score += 1.2;
                } else if snippet_lower.contains(clean_query) {
                    score += 0.6;
                }
            }

            // 4. Primary source / official documentation authority boost
            if let Some(domain) = r.display_domain.as_deref() {
                let d = domain.to_lowercase();
                if d.starts_with("docs.")
                    || d.starts_with("developer.")
                    || d == "github.com"
                    || d.ends_with(".github.io")
                    || d == "arxiv.org"
                    || d == "developer.mozilla.org"
                    || d == "wikipedia.org"
                {
                    score += 0.35;
                }
            }

            // 5. Content richness bonus
            if r.content.is_some() {
                score += 0.5;
            }
            if r.highlights.as_ref().is_some_and(|h| !h.is_empty()) {
                score += 0.3;
            }

            // 6. Freshness bonus
            if r.age.is_some() || r.fetched_at.is_some() {
                score += 0.1;
            }

            // 7. Position tie-breaker
            score -= (orig_idx as f64) * 0.05;

            (score, orig_idx)
        })
        .collect();

    // Sort by initial multi-signal score
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Domain diversity pass: apply slight dampening if the same domain dominates top results
    let mut domain_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut diversified: Vec<(f64, usize)> = Vec::with_capacity(scored.len());

    for (score, orig_idx) in scored {
        let mut final_score = score;
        if let Some(r) = results.get(orig_idx) {
            if let Some(domain) = r.display_domain.as_deref() {
                let count = domain_counts.entry(domain.to_lowercase()).or_insert(0);
                if *count >= 2 {
                    // Penalize 3rd and subsequent results from the same domain to encourage source diversity
                    final_score -= (*count as f64 - 1.0) * 0.25;
                }
                *count += 1;
            }
        }
        diversified.push((final_score, orig_idx));
    }

    diversified.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let orig = std::mem::take(results);
    let mut orig_map: std::collections::HashMap<usize, WebSearchResult> =
        orig.into_iter().enumerate().collect();

    for (new_rank, (final_score, orig_idx)) in diversified.into_iter().enumerate() {
        if let Some(mut r) = orig_map.remove(&orig_idx) {
            r.rank = new_rank + 1;
            r.score = Some((final_score * 100.0).round() / 100.0);
            r.ensure_provenance();
            if let Some(prov) = r.provenance.as_mut() {
                prov.ranking_stage = Some("multi_signal_ranked".into());
            }
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

    #[test]
    fn exact_phrase_matching_boosts_relevant_page() {
        let mut results = vec![
            WebSearchResult {
                id: "1".into(),
                title: "All about local first software".into(),
                url: "https://example.com/individual-words".into(),
                display_domain: Some("example.com".into()),
                snippet: Some("This page talks about local and first and software".into()),
                rank: 1,
                provider: Some("linkup".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "2".into(),
                title: "Architecture Guide".into(),
                url: "https://example.com/guide".into(),
                display_domain: Some("example.com".into()),
                snippet: Some(
                    "A comprehensive study on \"local-first architecture\" in 2026".into(),
                ),
                rank: 2,
                provider: Some("linkup".into()),
                ..Default::default()
            },
        ];

        rank_search_results(&mut results, "local-first architecture");
        assert_eq!(results[0].id, "2");
        assert_eq!(results[0].rank, 1);
    }

    #[test]
    fn domain_diversity_prevents_single_domain_monopoly() {
        let mut results = vec![
            WebSearchResult {
                id: "1".into(),
                title: "Example Article 1".into(),
                url: "https://monopoly.com/1".into(),
                display_domain: Some("monopoly.com".into()),
                snippet: Some("rust compiler performance insights".into()),
                rank: 1,
                provider: Some("linkup".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "2".into(),
                title: "Example Article 2".into(),
                url: "https://monopoly.com/2".into(),
                display_domain: Some("monopoly.com".into()),
                snippet: Some("rust compiler performance details".into()),
                rank: 2,
                provider: Some("linkup".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "3".into(),
                title: "Example Article 3".into(),
                url: "https://monopoly.com/3".into(),
                display_domain: Some("monopoly.com".into()),
                snippet: Some("rust compiler performance benchmarks".into()),
                rank: 3,
                provider: Some("linkup".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "4".into(),
                title: "Official Rust Performance Guide".into(),
                url: "https://docs.rust-lang.org/perf".into(),
                display_domain: Some("docs.rust-lang.org".into()),
                snippet: Some("rust compiler performance tuning".into()),
                rank: 4,
                provider: Some("exa".into()),
                ..Default::default()
            },
        ];

        rank_search_results(&mut results, "rust compiler performance");
        // Due to authority boost and diversity dampening on monopoly.com, the official docs source ranks high
        assert!(results.iter().take(2).any(|r| r.id == "4"));
    }
}
