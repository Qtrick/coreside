//! Provider-neutral web search entrypoints.

pub use super::registry::SearchRegistry;
pub use super::models::{WebSearchResponse, SafeSearchLevel};

use super::errors::SearchError;

pub async fn web_search(
    registry: &SearchRegistry,
    query: &str,
    count: usize,
    safe_search: SafeSearchLevel,
) -> Result<WebSearchResponse, SearchError> {
    registry.web_search(query, count, safe_search).await
}
