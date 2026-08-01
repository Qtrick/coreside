use std::sync::Arc;

use super::errors::SearchError;
use super::models::{ImageSearchResult, VideoSearchResult, WebSearchResult};
use super::normalization::{
    normalize_image_results, normalize_video_results, normalize_web_results,
};
use super::provider::{SearchProvider, SearchRequest};

/// Deterministic offline search provider for unit tests.
pub struct MockSearchProvider;

#[async_trait::async_trait]
impl SearchProvider for MockSearchProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn web_search(
        &self,
        req: &SearchRequest,
    ) -> Result<super::models::WebSearchResponse, SearchError> {
        let results = normalize_web_results(vec![WebSearchResult {
            id: "mock-web-1".into(),
            title: format!("Mock result for {}", req.query),
            url: "https://example.com/mock".into(),
            display_domain: Some("example.com".into()),
            snippet: Some("Mock snippet from offline provider.".into()),
            age: None,
            rank: 1,
        }]);
        Ok(super::models::WebSearchResponse {
            query: req.query.clone(),
            provider: self.id().to_string(),
            results,
            session_id: None,
            notice: None,
        })
    }

    async fn image_search(
        &self,
        req: &SearchRequest,
    ) -> Result<super::models::ImageSearchResponse, SearchError> {
        let results = normalize_image_results(vec![ImageSearchResult {
            id: "mock-img-1".into(),
            title: format!("Mock image for {}", req.query),
            page_url: "https://example.com/page".into(),
            image_url: "https://example.com/image.png".into(),
            thumbnail_url: Some("https://example.com/thumb.png".into()),
            width: Some(800),
            height: Some(600),
            source: Some("example.com".into()),
            rank: 1,
        }]);
        Ok(super::models::ImageSearchResponse {
            query: req.query.clone(),
            provider: self.id().to_string(),
            results,
            session_id: None,
            notice: None,
        })
    }

    async fn video_search(
        &self,
        req: &SearchRequest,
    ) -> Result<super::models::VideoSearchResponse, SearchError> {
        let results = normalize_video_results(vec![VideoSearchResult {
            id: "mock-vid-1".into(),
            title: format!("Mock video for {}", req.query),
            url: "https://example.com/video".into(),
            thumbnail_url: None,
            duration: Some("3:45".into()),
            creator: Some("Mock Creator".into()),
            age: None,
            rank: 1,
        }]);
        Ok(super::models::VideoSearchResponse {
            query: req.query.clone(),
            provider: self.id().to_string(),
            results,
            session_id: None,
            notice: None,
        })
    }

    async fn health_check(&self) -> Result<(), SearchError> {
        Ok(())
    }
}

pub fn mock_provider() -> Arc<dyn SearchProvider> {
    Arc::new(MockSearchProvider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{SafeSearchLevel, SearchRequest};

    #[tokio::test]
    async fn mock_web_normalization() {
        let provider = MockSearchProvider;
        let resp = provider
            .web_search(&SearchRequest {
                query: "rust".into(),
                count: 5,
                safe_search: SafeSearchLevel::Standard,
                country: None,
                domain: None,
            })
            .await
            .unwrap();
        assert_eq!(resp.provider, "mock");
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].rank, 1);
        assert!(resp.results[0].title.contains("rust"));
    }
}
