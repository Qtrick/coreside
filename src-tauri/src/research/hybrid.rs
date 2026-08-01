//! Hybrid web research: Exa discovery (when configured) + Crawl4AI enrichment.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::json;
use uuid::Uuid;

use crate::crawler::CrawlerSupervisor;
use crate::db::Database;
use crate::exa::{
    assert_can_spend, budget_status, estimate_search_cost, has_exa_key, load_search_profile,
    peek_cached_search, record_usage, search, SearchProfile,
};
use crate::search::{
    bound_text, normalize_image_results, normalize_video_results, normalize_web_results,
    validate_public_http_url, ImageSearchResponse, ImageSearchResult, SearchError, SearchProvider,
    SearchRequest, VideoSearchResponse, VideoSearchResult, WebSearchResponse, WebSearchResult,
};

use super::discovery::{classify_discovery_seed, DiscoverySeed, NEEDS_EXA_OR_SEED_MESSAGE};
use super::provider::Crawl4aiSearchProvider;
use super::ranking::rank_web_candidates;

const PROVIDER_ID: &str = "hybrid";

pub struct HybridSearchProvider {
    crawl: Crawl4aiSearchProvider,
    supervisor: Arc<CrawlerSupervisor>,
    db: Arc<Mutex<Database>>,
}

impl HybridSearchProvider {
    pub fn new(supervisor: Arc<CrawlerSupervisor>, db: Arc<Mutex<Database>>) -> Self {
        Self {
            crawl: Crawl4aiSearchProvider::new(supervisor.clone()),
            supervisor,
            db,
        }
    }

    pub async fn fetch_page(
        &self,
        raw_url: &str,
    ) -> Result<crate::search::FetchedWebPage, SearchError> {
        self.crawl.fetch_page(raw_url).await
    }
}

#[async_trait]
impl SearchProvider for HybridSearchProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    async fn web_search(&self, req: &SearchRequest) -> Result<WebSearchResponse, SearchError> {
        let count = req.count.clamp(1, 20);
        match classify_discovery_seed(&req.query, req.domain.as_deref()) {
            DiscoverySeed::NeedsSeed => self.free_text_search(req, count).await,
            DiscoverySeed::Url(url) => {
                // Direct URL → Crawl4AI only (no Exa).
                let mut inner = req.clone();
                inner.query = url;
                let mut resp = self.crawl.web_search(&inner).await?;
                resp.provider = PROVIDER_ID.into();
                Ok(resp)
            }
            DiscoverySeed::Domain(domain) => {
                // Prefer local domain discovery; Exa includeDomains is optional fallback
                // when discovery returns empty and Exa is configured.
                let payload = self
                    .supervisor
                    .send_command(
                        "discover_domain",
                        json!({
                            "domain": domain,
                            "query": req.query,
                            "limit": count,
                        }),
                    )
                    .await;
                match payload {
                    Ok(payload) => {
                        let candidates = payload
                            .get("candidates")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let results = rank_web_candidates(&candidates, &req.query, count);
                        if results.is_empty() && has_exa_key() {
                            return self
                                .exa_discover(req, count, Some(vec![domain.clone()]))
                                .await;
                        }
                        Ok(WebSearchResponse {
                            query: req.query.clone(),
                            provider: PROVIDER_ID.into(),
                            results,
                            session_id: None,
                            notice: None,
                        })
                    }
                    Err(_) if has_exa_key() => {
                        self.exa_discover(req, count, Some(vec![domain])).await
                    }
                    Err(e) => Err(crate::crawler::to_search_error(e)),
                }
            }
        }
    }

    async fn image_search(&self, req: &SearchRequest) -> Result<ImageSearchResponse, SearchError> {
        // Media still needs a crawlable seed; Exa free-text → crawl top pages then extract.
        match classify_discovery_seed(&req.query, req.domain.as_deref()) {
            DiscoverySeed::NeedsSeed if !has_exa_key() => Ok(ImageSearchResponse {
                query: req.query.clone(),
                provider: PROVIDER_ID.into(),
                results: vec![],
                session_id: None,
                notice: Some(NEEDS_EXA_OR_SEED_MESSAGE.into()),
            }),
            DiscoverySeed::NeedsSeed => {
                let web = self.free_text_search(req, 3).await?;
                if web.results.is_empty() {
                    return Ok(ImageSearchResponse {
                        query: req.query.clone(),
                        provider: PROVIDER_ID.into(),
                        results: vec![],
                        session_id: None,
                        notice: web.notice,
                    });
                }
                let mut results = Vec::new();
                for (i, r) in web.results.iter().take(2).enumerate() {
                    let mut seeded = req.clone();
                    seeded.query = r.url.clone();
                    if let Ok(media) = self.crawl.image_search(&seeded).await {
                        for img in media.results {
                            results.push(ImageSearchResult {
                                id: format!("hybrid-img-{}-{}", i, img.id),
                                ..img
                            });
                        }
                    }
                }
                Ok(ImageSearchResponse {
                    query: req.query.clone(),
                    provider: PROVIDER_ID.into(),
                    results: normalize_image_results(results),
                    session_id: None,
                    notice: None,
                })
            }
            _ => {
                let mut resp = self.crawl.image_search(req).await?;
                resp.provider = PROVIDER_ID.into();
                Ok(resp)
            }
        }
    }

    async fn video_search(&self, req: &SearchRequest) -> Result<VideoSearchResponse, SearchError> {
        match classify_discovery_seed(&req.query, req.domain.as_deref()) {
            DiscoverySeed::NeedsSeed if !has_exa_key() => Ok(VideoSearchResponse {
                query: req.query.clone(),
                provider: PROVIDER_ID.into(),
                results: vec![],
                session_id: None,
                notice: Some(NEEDS_EXA_OR_SEED_MESSAGE.into()),
            }),
            DiscoverySeed::NeedsSeed => {
                let web = self.free_text_search(req, 3).await?;
                if web.results.is_empty() {
                    return Ok(VideoSearchResponse {
                        query: req.query.clone(),
                        provider: PROVIDER_ID.into(),
                        results: vec![],
                        session_id: None,
                        notice: web.notice,
                    });
                }
                let mut results = Vec::new();
                for (i, r) in web.results.iter().take(2).enumerate() {
                    let mut seeded = req.clone();
                    seeded.query = r.url.clone();
                    if let Ok(media) = self.crawl.video_search(&seeded).await {
                        for vid in media.results {
                            results.push(VideoSearchResult {
                                id: format!("hybrid-vid-{}-{}", i, vid.id),
                                ..vid
                            });
                        }
                    }
                }
                Ok(VideoSearchResponse {
                    query: req.query.clone(),
                    provider: PROVIDER_ID.into(),
                    results: normalize_video_results(results),
                    session_id: None,
                    notice: None,
                })
            }
            _ => {
                let mut resp = self.crawl.video_search(req).await?;
                resp.provider = PROVIDER_ID.into();
                Ok(resp)
            }
        }
    }

    async fn health_check(&self) -> Result<(), SearchError> {
        // Prefer local engine; Exa connectivity is separate (test_exa_connection).
        self.crawl.health_check().await
    }
}

impl HybridSearchProvider {
    async fn free_text_search(
        &self,
        req: &SearchRequest,
        count: usize,
    ) -> Result<WebSearchResponse, SearchError> {
        if !has_exa_key() {
            return Ok(WebSearchResponse {
                query: req.query.clone(),
                provider: PROVIDER_ID.into(),
                results: vec![],
                session_id: None,
                notice: Some(NEEDS_EXA_OR_SEED_MESSAGE.into()),
            });
        }
        self.exa_discover(req, count, None).await
    }

    async fn exa_discover(
        &self,
        req: &SearchRequest,
        count: usize,
        include_domains: Option<Vec<String>>,
    ) -> Result<WebSearchResponse, SearchError> {
        let profile = {
            let db = self.db.lock();
            let configured = load_search_profile(&db);
            let status = budget_status(&db);
            // Soft: Thorough→Balanced; Critical/Hard: Saver (Hard still blocked by assert_can_spend).
            configured.effective_for_threshold(status.threshold)
        };
        let num = profile.num_results().min(count).min(10);
        let domains_ref = include_domains.as_deref();

        // Cache hits skip budget and do not attribute spend.
        let outcome = if let Some(cached) = peek_cached_search(&req.query, profile, domains_ref) {
            cached
        } else {
            let estimated = estimate_search_cost(profile.search_type(), num);
            {
                let db = self.db.lock();
                assert_can_spend(&db, estimated)?;
            }
            search(&req.query, profile, include_domains).await?
        };

        let mut results = map_exa_results(&outcome.response.results, num);

        // Persist usage (actual from costDollars when present; cache hits = $0).
        let (actual_cost, estimated_cost) = if outcome.cache_hit {
            (Some(0.0), Some(0.0))
        } else {
            (outcome.response.actual_cost(), Some(outcome.estimated_cost))
        };
        {
            let mut db = self.db.lock();
            let _ = record_usage(
                &mut db,
                &outcome.search_mode,
                results.len(),
                outcome.response.request_id.as_deref(),
                actual_cost,
                estimated_cost,
                outcome.cache_hit,
                "ok",
                None,
            );
        }

        // Optionally enrich top N via Crawl4AI; keep Exa highlights if crawl fails.
        let crawl_n = profile.max_crawl_pages().min(results.len());
        if crawl_n > 0 {
            results = enrich_with_crawl(&self.crawl, results, crawl_n).await;
        }

        Ok(WebSearchResponse {
            query: req.query.clone(),
            provider: PROVIDER_ID.into(),
            results: normalize_web_results(results),
            session_id: None,
            notice: None,
        })
    }
}

fn map_exa_results(results: &[crate::exa::ExaResult], limit: usize) -> Vec<WebSearchResult> {
    let mut out = Vec::new();
    for (i, r) in results.iter().take(limit).enumerate() {
        let Ok(safe_url) = validate_public_http_url(&r.url) else {
            continue;
        };
        let title = r
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| safe_url.as_str().to_string());
        let snippet = r.snippet().map(|s| bound_text(&s, 400));
        let domain = safe_url.host_str().map(|h| h.to_string());
        out.push(WebSearchResult {
            id: r
                .id
                .clone()
                .unwrap_or_else(|| format!("exa-{}", Uuid::new_v4())),
            title,
            url: safe_url.to_string(),
            display_domain: domain,
            snippet,
            age: r.published_date.clone(),
            rank: i + 1,
        });
    }
    out
}

async fn enrich_with_crawl(
    crawl: &Crawl4aiSearchProvider,
    mut results: Vec<WebSearchResult>,
    crawl_n: usize,
) -> Vec<WebSearchResult> {
    for result in results.iter_mut().take(crawl_n) {
        let Ok(page) = crawl.fetch_page(&result.url).await else {
            continue;
        };
        if result.title.trim().is_empty() || result.title == result.url {
            if let Some(t) = page.title.filter(|t| !t.trim().is_empty()) {
                result.title = t;
            }
        }
        let excerpt = bound_text(&page.text, 400);
        if !excerpt.trim().is_empty() {
            result.snippet = Some(excerpt);
        }
    }
    results
}

/// Honest notices for agent / UI when open-web search is unavailable.
pub fn research_capability_notice(exa_configured: bool) -> String {
    if exa_configured {
        "Open-web free-text search is available via Exa. You may call web_search with a natural-language query. Direct URLs still crawl via Crawl4AI only."
            .into()
    } else {
        format!(
            "{NEEDS_EXA_OR_SEED_MESSAGE} Until Exa is configured, only URL or domain seeds work."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_mentions_exa_or_url() {
        assert!(NEEDS_EXA_OR_SEED_MESSAGE.contains("Exa"));
        assert!(
            NEEDS_EXA_OR_SEED_MESSAGE.contains("URL") || NEEDS_EXA_OR_SEED_MESSAGE.contains("url")
        );
    }

    #[test]
    fn capability_notice_switches() {
        let with = research_capability_notice(true);
        assert!(with.contains("Exa"));
        assert!(with.to_lowercase().contains("free-text") || with.contains("natural-language"));
        let without = research_capability_notice(false);
        assert!(without.contains("Exa") || without.contains("URL"));
    }

    #[test]
    fn saver_crawl_bound() {
        assert_eq!(SearchProfile::Saver.max_crawl_pages(), 2);
    }
}
