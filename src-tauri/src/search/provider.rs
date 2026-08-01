use async_trait::async_trait;

use super::errors::SearchError;
use super::models::{ImageSearchResponse, SafeSearchLevel, VideoSearchResponse, WebSearchResponse};

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub count: usize,
    pub safe_search: SafeSearchLevel,
    pub country: Option<String>,
    /// Optional domain seed for local Crawl4AI discovery (not a SERP country code).
    pub domain: Option<String>,
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;

    async fn web_search(&self, req: &SearchRequest) -> Result<WebSearchResponse, SearchError>;

    async fn image_search(&self, req: &SearchRequest) -> Result<ImageSearchResponse, SearchError>;

    async fn video_search(&self, req: &SearchRequest) -> Result<VideoSearchResponse, SearchError>;

    async fn health_check(&self) -> Result<(), SearchError>;
}
