use std::sync::Arc;

use parking_lot::Mutex;

use super::errors::SearchError;
use super::mock::mock_provider;
use super::models::{SafeSearchLevel, WebSearchResponse};
use super::provider::{SearchProvider, SearchRequest};
use crate::crawler::CrawlerSupervisor;
use crate::db::Database;
use crate::research::HybridSearchProvider;

pub struct SearchRegistry {
    provider: Arc<dyn SearchProvider>,
}

impl SearchRegistry {
    pub fn from_provider(provider: Arc<dyn SearchProvider>) -> Self {
        Self { provider }
    }

    /// Default production provider: Exa (when configured) + Crawl4AI hybrid.
    pub fn default_local(
        supervisor: Arc<CrawlerSupervisor>,
        db: Arc<Mutex<Database>>,
    ) -> Self {
        Self::from_provider(Arc::new(HybridSearchProvider::new(supervisor, db)))
    }

    pub fn mock() -> Self {
        Self::from_provider(mock_provider())
    }

    pub fn provider_id(&self) -> &str {
        self.provider.id()
    }

    pub async fn web_search(
        &self,
        query: &str,
        count: usize,
        safe_search: SafeSearchLevel,
    ) -> Result<WebSearchResponse, SearchError> {
        let req = SearchRequest {
            query: query.trim().to_string(),
            count,
            safe_search,
            country: None,
            domain: None,
        };
        if req.query.is_empty() {
            return Err(SearchError::Invalid("query is required".into()));
        }
        self.provider.web_search(&req).await
    }

    pub async fn image_search(
        &self,
        query: &str,
        count: usize,
        safe_search: SafeSearchLevel,
    ) -> Result<super::models::ImageSearchResponse, SearchError> {
        let req = SearchRequest {
            query: query.trim().to_string(),
            count,
            safe_search,
            country: None,
            domain: None,
        };
        if req.query.is_empty() {
            return Err(SearchError::Invalid("query is required".into()));
        }
        self.provider.image_search(&req).await
    }

    pub async fn video_search(
        &self,
        query: &str,
        count: usize,
        safe_search: SafeSearchLevel,
    ) -> Result<super::models::VideoSearchResponse, SearchError> {
        let req = SearchRequest {
            query: query.trim().to_string(),
            count,
            safe_search,
            country: None,
            domain: None,
        };
        if req.query.is_empty() {
            return Err(SearchError::Invalid("query is required".into()));
        }
        self.provider.video_search(&req).await
    }

    pub async fn health_check(&self) -> Result<(), SearchError> {
        self.provider.health_check().await
    }

    /// Domain-seeded discovery (optional explicit domain separate from query text).
    pub async fn web_search_with_domain(
        &self,
        query: &str,
        domain: Option<&str>,
        count: usize,
        safe_search: SafeSearchLevel,
    ) -> Result<WebSearchResponse, SearchError> {
        let domain = domain.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let query = query.trim().to_string();
        if query.is_empty() && domain.is_none() {
            return Err(SearchError::Invalid("query or domain is required".into()));
        }
        let req = SearchRequest {
            query: if query.is_empty() {
                domain.clone().unwrap_or_default()
            } else {
                query
            },
            count,
            safe_search,
            country: None,
            domain,
        };
        self.provider.web_search(&req).await
    }
}
