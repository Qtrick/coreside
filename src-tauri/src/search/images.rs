pub use super::models::{ImageSearchResponse, SafeSearchLevel};
pub use super::registry::SearchRegistry;

use super::errors::SearchError;

pub async fn image_search(
    registry: &SearchRegistry,
    query: &str,
    count: usize,
    safe_search: SafeSearchLevel,
) -> Result<ImageSearchResponse, SearchError> {
    registry.image_search(query, count, safe_search).await
}
