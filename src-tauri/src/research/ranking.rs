//! Lightweight ranking helpers for discovery candidates.

use serde_json::Value;

use crate::search::{normalize_web_results, validate_public_http_url, WebSearchResult};

/// Convert sidecar discovery candidates into ranked web search results.
pub fn rank_web_candidates(
    candidates: &[Value],
    query: &str,
    limit: usize,
) -> Vec<WebSearchResult> {
    let mut results: Vec<WebSearchResult> = Vec::new();
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
        let domain = safe_url.host_str().map(|h| h.to_string());
        results.push(WebSearchResult {
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
            rank: i + 1,
            provider: Some("crawl4ai".into()),
            score: c.get("score").and_then(|v| v.as_f64()),
            ..Default::default()
        });
    }

    rank_search_results(&mut results, query);
    results.truncate(limit.max(1));
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

            // 1. Normalized upstream provider confidence (bounded to [0.0, 0.3], no arbitrary vendor bias)
            // If the provider supplies a relevance score, normalize to [0.0, 1.0]; otherwise use neutral 0.5.
            let provider_conf = r
                .score
                .map(|s| {
                    if (0.0..=1.0).contains(&s) {
                        s
                    } else if s > 1.0 {
                        (s / 100.0).clamp(0.0, 1.0)
                    } else {
                        0.5
                    }
                })
                .unwrap_or(0.5);
            score += provider_conf * 0.3;

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

    #[test]
    fn benchmark_corpus_covers_eight_query_classes() {
        // 1. Documentation query: official docs should rank top
        let mut doc_results = vec![
            WebSearchResult {
                id: "blog".into(),
                title: "How I use Arc in my project".into(),
                url: "https://myblog.dev/arc".into(),
                display_domain: Some("myblog.dev".into()),
                snippet: Some("A blog post about Arc pointers".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "official".into(),
                title: "std::sync::Arc - Rust".into(),
                url: "https://docs.rust-lang.org/std/sync/struct.Arc.html".into(),
                display_domain: Some("docs.rust-lang.org".into()),
                snippet: Some("Thread-safe reference-counting pointer.".into()),
                ..Default::default()
            },
        ];
        rank_search_results(&mut doc_results, "rust std::sync::Arc documentation");
        assert_eq!(doc_results[0].id, "official");

        // 2. Research query: academic source / arxiv should be boosted
        let mut research_results = vec![
            WebSearchResult {
                id: "general".into(),
                title: "Transformers are cool".into(),
                url: "https://medium.com/transformers".into(),
                display_domain: Some("medium.com".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "paper".into(),
                title: "Efficient Transformer Attention Mechanisms".into(),
                url: "https://arxiv.org/abs/2009.06732".into(),
                display_domain: Some("arxiv.org".into()),
                snippet: Some(
                    "A survey of attention mechanisms and computational efficiency.".into(),
                ),
                ..Default::default()
            },
        ];
        rank_search_results(
            &mut research_results,
            "transformer attention mechanisms efficiency",
        );
        assert_eq!(research_results[0].id, "paper");

        // 3. Current news query: freshness signal contributes
        let mut news_results = vec![
            WebSearchResult {
                id: "old".into(),
                title: "Browser release history from 2018".into(),
                url: "https://example.com/2018".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "fresh".into(),
                title: "Latest Browser Release Updates".into(),
                url: "https://example.com/latest".into(),
                snippet: Some("New version release notes and updates.".into()),
                age: Some("2 hours ago".into()),
                fetched_at: Some("2026-09-17T12:00:00Z".into()),
                ..Default::default()
            },
        ];
        rank_search_results(&mut news_results, "latest browser release updates");
        assert_eq!(news_results[0].id, "fresh");

        // 4. Known company query: exact phrase and domain match
        let mut company_results = vec![
            WebSearchResult {
                id: "unrelated".into(),
                title: "History of the name Claude".into(),
                url: "https://names.org/claude".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "company".into(),
                title: "Anthropic Claude Architecture".into(),
                url: "https://anthropic.com/research/claude".into(),
                display_domain: Some("anthropic.com".into()),
                snippet: Some(
                    "Detailed technical report on the Anthropic Claude architecture.".into(),
                ),
                ..Default::default()
            },
        ];
        rank_search_results(&mut company_results, "Anthropic Claude Architecture");
        assert_eq!(company_results[0].id, "company");

        // 5. Technical question: exact phrase in snippet boosts resolution
        let mut tech_results = vec![
            WebSearchResult {
                id: "generic".into(),
                title: "SQL Databases".into(),
                url: "https://example.com/sql".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "specific".into(),
                title: "SQLite Foreign Key Support".into(),
                url: "https://sqlite.org/foreignkeys.html".into(),
                display_domain: Some("sqlite.org".into()),
                snippet: Some(
                    "How to configure sqlite foreign key cascade on delete actions.".into(),
                ),
                ..Default::default()
            },
        ];
        rank_search_results(&mut tech_results, "sqlite foreign key cascade on delete");
        assert_eq!(tech_results[0].id, "specific");

        // 6. Domain-specific query: URL / domain terms match
        let mut domain_results = vec![
            WebSearchResult {
                id: "other".into(),
                title: "Tauri Commands Guide".into(),
                url: "https://random-forum.com/t/123".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "docs_rs".into(),
                title: "tauri::command in tauri - Rust".into(),
                url: "https://docs.rs/tauri/latest/tauri/attr.command.html".into(),
                display_domain: Some("docs.rs".into()),
                ..Default::default()
            },
        ];
        rank_search_results(&mut domain_results, "site:docs.rs tauri commands");
        assert_eq!(domain_results[0].id, "docs_rs");

        // 7. Multi-source comparison: query terms in title & snippet
        let mut compare_results = vec![
            WebSearchResult {
                id: "single".into(),
                title: "All about PostgreSQL".into(),
                url: "https://example.com/postgres".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "comparison".into(),
                title: "PostgreSQL vs SQLite for Local Embedded Storage".into(),
                url: "https://example.com/compare".into(),
                snippet: Some("In-depth trade-off comparison between postgresql and sqlite for local embedded use.".into()),
                ..Default::default()
            },
        ];
        rank_search_results(&mut compare_results, "postgresql vs sqlite local embedded");
        assert_eq!(compare_results[0].id, "comparison");

        // 8. Ambiguous query: content availability & authority resolve precedence
        let mut ambig_results = vec![
            WebSearchResult {
                id: "metal".into(),
                title: "Rust oxide on steel".into(),
                url: "https://metals.com/rust".into(),
                ..Default::default()
            },
            WebSearchResult {
                id: "lang".into(),
                title: "Rust Programming Language".into(),
                url: "https://www.rust-lang.org/".into(),
                display_domain: Some("rust-lang.org".into()),
                content: Some(
                    "A language empowering everyone to build reliable and efficient software."
                        .into(),
                ),
                ..Default::default()
            },
        ];
        rank_search_results(&mut ambig_results, "rust");
        assert_eq!(ambig_results[0].id, "lang");

        // 9. Navigational query: official root domain lookup
        let mut nav_results = vec![
            WebSearchResult {
                id: "third_party_guide".into(),
                title: "Getting Started with Tauri Framework".into(),
                url: "https://tutorialspoint.com/tauri-getting-started".into(),
                display_domain: Some("tutorialspoint.com".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "official_home".into(),
                title: "Tauri Apps: Build smaller, faster, and more secure desktop applications".into(),
                url: "https://tauri.app/".into(),
                display_domain: Some("tauri.app".into()),
                snippet: Some("Tauri is an app construction toolkit that lets you build desktop applications.".into()),
                ..Default::default()
            },
        ];
        rank_search_results(&mut nav_results, "tauri app homepage");
        assert_eq!(nav_results[0].id, "official_home");

        // 10. Long-tail conversational technical query
        let mut longtail_results = vec![
            WebSearchResult {
                id: "broad".into(),
                title: "Async programming in Rust".into(),
                url: "https://example.com/rust-async".into(),
                display_domain: Some("example.com".into()),
                snippet: Some("Overview of futures and tasks in rust async.".into()),
                ..Default::default()
            },
            WebSearchResult {
                id: "precise_match".into(),
                title: "Handling backpressure and unbounded memory growth in tokio channels".into(),
                url: "https://tokio.rs/blog/backpressure".into(),
                display_domain: Some("tokio.rs".into()),
                snippet: Some("How to prevent unbounded memory growth by configuring bounded mpsc channel backpressure.".into()),
                ..Default::default()
            },
        ];
        rank_search_results(
            &mut longtail_results,
            "how to avoid unbounded memory growth with backpressure in tokio channels",
        );
        assert_eq!(longtail_results[0].id, "precise_match");

        // Aggregate Quantitative Evaluation: Compute MRR and Top-1 across all 10 query classes
        let test_cases = [
            (&doc_results, "official"),
            (&research_results, "paper"),
            (&news_results, "fresh"),
            (&company_results, "company"),
            (&tech_results, "specific"),
            (&domain_results, "docs_rs"),
            (&compare_results, "comparison"),
            (&ambig_results, "lang"),
            (&nav_results, "official_home"),
            (&longtail_results, "precise_match"),
        ];

        let mut reciprocal_ranks = Vec::new();
        let mut top_1_hits = 0;

        for (results, expected_id) in &test_cases {
            if let Some(pos) = results.iter().position(|r| r.id == *expected_id) {
                let rank = pos + 1;
                reciprocal_ranks.push(1.0 / (rank as f64));
                if rank == 1 {
                    top_1_hits += 1;
                }
            } else {
                reciprocal_ranks.push(0.0);
            }
        }

        let mrr: f64 = reciprocal_ranks.iter().sum::<f64>() / (test_cases.len() as f64);
        let top_1_acc = (top_1_hits as f64) / (test_cases.len() as f64);

        assert!(mrr >= 0.95, "MRR must be >= 0.95, got {mrr:.3}");
        assert!(
            top_1_acc >= 0.90,
            "Top-1 accuracy must be >= 0.90, got {top_1_acc:.3}"
        );
    }
}
