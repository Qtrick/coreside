pub use super::models::{SafeSearchLevel, VideoSearchResponse};
pub use super::registry::SearchRegistry;

use super::errors::SearchError;

pub async fn video_search(
    registry: &SearchRegistry,
    query: &str,
    count: usize,
    safe_search: SafeSearchLevel,
) -> Result<VideoSearchResponse, SearchError> {
    registry.video_search(query, count, safe_search).await
}
