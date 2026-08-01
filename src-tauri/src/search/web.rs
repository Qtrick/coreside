//! Provider-neutral web search entrypoints.

pub use super::models::{SafeSearchLevel, WebSearchResponse};
pub use super::registry::SearchRegistry;

use super::errors::SearchError;

pub async fn web_search(
    registry: &SearchRegistry,
    query: &str,
    count: usize,
    safe_search: SafeSearchLevel,
) -> Result<WebSearchResponse, SearchError> {
    registry.web_search(query, count, safe_search).await
}
