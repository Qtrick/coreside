//! Crawl4AI-backed SearchProvider — replaces Brave for local research.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::crawler::{to_search_error, CrawlerSupervisor};
use crate::search::{
    bound_text, normalize_image_results, normalize_video_results, normalize_web_results,
    validate_public_http_url, FetchedWebPage, ImageSearchResponse, ImageSearchResult,
    SearchError, SearchProvider, SearchRequest, VideoSearchResponse, VideoSearchResult,
    WebSearchResponse, WebSearchResult,
};

use super::discovery::{classify_discovery_seed, DiscoverySeed, NEEDS_SEED_MESSAGE};
use super::ranking::rank_web_candidates;

const PROVIDER_ID: &str = "crawl4ai";
const MAX_EXTRACTED_TEXT_CHARS: usize = 32_000;

pub struct Crawl4aiSearchProvider {
    supervisor: Arc<CrawlerSupervisor>,
}

impl Crawl4aiSearchProvider {
    pub fn new(supervisor: Arc<CrawlerSupervisor>) -> Self {
        Self { supervisor }
    }

    pub async fn fetch_page(&self, raw_url: &str) -> Result<FetchedWebPage, SearchError> {
        let url = validate_public_http_url(raw_url)?;
        let payload = self
            .supervisor
            .send_command(
                "crawl_url",
                json!({
                    "url": url.as_str(),
                    "profile": "text",
                }),
            )
            .await
            .map_err(to_search_error)?;
        page_from_crawl_payload(raw_url, &payload)
    }
}

#[async_trait]
impl SearchProvider for Crawl4aiSearchProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    async fn web_search(&self, req: &SearchRequest) -> Result<WebSearchResponse, SearchError> {
        let count = req.count.clamp(1, 20);
        match classify_discovery_seed(&req.query, req.domain.as_deref()) {
            DiscoverySeed::NeedsSeed => Ok(WebSearchResponse {
                query: req.query.clone(),
                provider: PROVIDER_ID.into(),
                results: vec![],
                session_id: None,
                notice: Some(NEEDS_SEED_MESSAGE.into()),
            }),
            DiscoverySeed::Url(url) => {
                validate_public_http_url(&url)?;
                let payload = self
                    .supervisor
                    .send_command(
                        "crawl_url",
                        json!({
                            "url": url,
                            "profile": "text",
                        }),
                    )
                    .await
                    .map_err(to_search_error)?;
                let result = web_result_from_crawl(&payload, 1)?;
                Ok(WebSearchResponse {
                    query: req.query.clone(),
                    provider: PROVIDER_ID.into(),
                    results: normalize_web_results(vec![result]),
                    session_id: None,
                    notice: None,
                })
            }
            DiscoverySeed::Domain(domain) => {
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
                    .await
                    .map_err(to_search_error)?;
                let candidates = payload
                    .get("candidates")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let results = rank_web_candidates(&candidates, &req.query, count);
                Ok(WebSearchResponse {
                    query: req.query.clone(),
                    provider: PROVIDER_ID.into(),
                    results,
                    session_id: None,
                    notice: None,
                })
            }
        }
    }

    async fn image_search(&self, req: &SearchRequest) -> Result<ImageSearchResponse, SearchError> {
        let count = req.count.clamp(1, 40);
        let media = self.extract_media_for_seed(req).await?;
        let images = media
            .get("images")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut results = Vec::new();
        for (i, img) in images.into_iter().take(count).enumerate() {
            let image_url = img
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if image_url.is_empty() {
                continue;
            }
            let page_url = img
                .get("sourcePageUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = img
                .get("alt")
                .or_else(|| img.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("Image")
                .to_string();
            results.push(ImageSearchResult {
                id: format!("crawl-img-{}", i + 1),
                title,
                page_url: if page_url.is_empty() {
                    image_url.clone()
                } else {
                    page_url
                },
                image_url,
                thumbnail_url: None,
                width: img.get("width").and_then(|v| v.as_u64()).map(|n| n as u32),
                height: img.get("height").and_then(|v| v.as_u64()).map(|n| n as u32),
                source: img
                    .get("sourcePageUrl")
                    .and_then(|v| v.as_str())
                    .and_then(|u| {
                        url::Url::parse(u)
                            .ok()
                            .and_then(|p| p.host_str().map(|h| h.to_string()))
                    }),
                rank: 0,
            });
        }
        let notice = if results.is_empty() {
            match classify_discovery_seed(&req.query, req.domain.as_deref()) {
                DiscoverySeed::NeedsSeed => Some(NEEDS_SEED_MESSAGE.into()),
                _ => None,
            }
        } else {
            None
        };
        Ok(ImageSearchResponse {
            query: req.query.clone(),
            provider: PROVIDER_ID.into(),
            results: normalize_image_results(results),
            session_id: None,
            notice,
        })
    }

    async fn video_search(&self, req: &SearchRequest) -> Result<VideoSearchResponse, SearchError> {
        let count = req.count.clamp(1, 40);
        let media = self.extract_media_for_seed(req).await?;
        let videos = media
            .get("videos")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut results = Vec::new();
        for (i, vid) in videos.into_iter().take(count).enumerate() {
            let url = vid
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if url.is_empty() {
                continue;
            }
            let title = vid
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Video")
                .to_string();
            results.push(VideoSearchResult {
                id: format!("crawl-vid-{}", i + 1),
                title,
                url,
                thumbnail_url: vid
                    .get("thumbnailUrl")
                    .or_else(|| vid.get("poster"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                duration: None,
                creator: None,
                age: None,
                rank: 0,
            });
        }
        let notice = if results.is_empty() {
            match classify_discovery_seed(&req.query, req.domain.as_deref()) {
                DiscoverySeed::NeedsSeed => Some(NEEDS_SEED_MESSAGE.into()),
                _ => None,
            }
        } else {
            None
        };
        Ok(VideoSearchResponse {
            query: req.query.clone(),
            provider: PROVIDER_ID.into(),
            results: normalize_video_results(results),
            session_id: None,
            notice,
        })
    }

    async fn health_check(&self) -> Result<(), SearchError> {
        self.supervisor
            .send_command("health", json!({}))
            .await
            .map_err(to_search_error)?;
        Ok(())
    }
}

impl Crawl4aiSearchProvider {
    async fn extract_media_for_seed(&self, req: &SearchRequest) -> Result<Value, SearchError> {
        match classify_discovery_seed(&req.query, req.domain.as_deref()) {
            DiscoverySeed::NeedsSeed => Ok(json!({"images": [], "videos": []})),
            DiscoverySeed::Url(url) => {
                validate_public_http_url(&url)?;
                let payload = self
                    .supervisor
                    .send_command(
                        "extract_media",
                        json!({
                            "url": url,
                            "profile": "media",
                        }),
                    )
                    .await
                    .map_err(to_search_error)?;
                Ok(payload
                    .get("media")
                    .cloned()
                    .unwrap_or_else(|| json!({"images": [], "videos": []})))
            }
            DiscoverySeed::Domain(domain) => {
                let seed_url = format!("https://{domain}/");
                validate_public_http_url(&seed_url)?;
                let payload = self
                    .supervisor
                    .send_command(
                        "extract_media",
                        json!({
                            "url": seed_url,
                            "profile": "media",
                        }),
                    )
                    .await
                    .map_err(to_search_error)?;
                Ok(payload
                    .get("media")
                    .cloned()
                    .unwrap_or_else(|| json!({"images": [], "videos": []})))
            }
        }
    }
}

fn web_result_from_crawl(payload: &Value, rank: usize) -> Result<WebSearchResult, SearchError> {
    let source = payload.get("source").unwrap_or(payload);
    let url = source
        .get("finalUrl")
        .or_else(|| source.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if url.is_empty() {
        return Err(SearchError::Provider("crawl returned no url".into()));
    }
    // Defense in depth: never surface a private/final redirected URL.
    let safe_url = validate_public_http_url(&url)?;
    let title = source
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(safe_url.as_str())
        .to_string();
    let snippet = payload
        .pointer("/content/excerpt")
        .or_else(|| source.get("excerpt"))
        .and_then(|v| v.as_str())
        .map(|s| bound_text(s, 400));
    let domain = safe_url.host_str().map(|h| h.to_string());
    Ok(WebSearchResult {
        id: format!("crawl-{}", Uuid::new_v4()),
        title,
        url: safe_url.to_string(),
        display_domain: domain,
        snippet,
        age: None,
        rank,
    })
}

fn page_from_crawl_payload(raw_url: &str, payload: &Value) -> Result<FetchedWebPage, SearchError> {
    let source = payload.get("source").unwrap_or(payload);
    let final_url = source
        .get("finalUrl")
        .or_else(|| source.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or(raw_url)
        .to_string();
    // Re-validate final URL (SSRF redirect safety).
    validate_public_http_url(&final_url)?;
    let title = source
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let text = payload
        .pointer("/content/markdown")
        .or_else(|| payload.pointer("/content/excerpt"))
        .or_else(|| source.get("markdown"))
        .or_else(|| source.get("excerpt"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let byte_size = text.len();
    Ok(FetchedWebPage {
        url: raw_url.trim().to_string(),
        final_url,
        title,
        text: bound_text(&text, MAX_EXTRACTED_TEXT_CHARS),
        content_type: Some("text/markdown".into()),
        byte_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::validate_public_http_url;
    use serde_json::json;

    #[test]
    fn fetch_path_still_uses_ssrf_safety() {
        assert!(validate_public_http_url("http://127.0.0.1/").is_err());
        assert!(validate_public_http_url("https://1.1.1.1/").is_ok());
    }

    #[test]
    fn needs_seed_message_is_clear() {
        assert!(NEEDS_SEED_MESSAGE.contains("URL"));
        assert!(NEEDS_SEED_MESSAGE.contains("domain") || NEEDS_SEED_MESSAGE.contains("site"));
    }

    #[test]
    fn page_from_payload_revalidates_final_url() {
        let payload = json!({
            "source": {
                "url": "https://1.1.1.1/",
                "finalUrl": "http://127.0.0.1/secret",
                "title": "Nope",
                "excerpt": "x"
            },
            "content": { "markdown": "hello", "excerpt": "hello" }
        });
        let err = page_from_crawl_payload("https://1.1.1.1/", &payload).unwrap_err();
        assert_eq!(err.code(), "ssrf_blocked");
    }
}
