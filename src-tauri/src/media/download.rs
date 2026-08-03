use futures_util::StreamExt;
use reqwest::Client;
use tokio::time::{timeout, Duration};

use crate::search::{send_public_get, SearchError, MAX_FETCH_BYTES};

use super::errors::MediaError;
use super::limits::max_bytes_for_category;
use super::validation::{content_hash, validate_bytes};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Stream a response body while enforcing a hard byte budget (decompressed).
pub async fn read_body_bounded(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, MediaError> {
    if let Some(len) = response.content_length() {
        if len as usize > max_bytes {
            return Err(MediaError::Invalid(format!(
                "declared Content-Length {len} exceeds {max_bytes} bytes"
            )));
        }
    }

    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| MediaError::Download(e.to_string()))?;
        if out.len().saturating_add(chunk.len()) > max_bytes {
            return Err(MediaError::Invalid(format!(
                "download exceeds {max_bytes} bytes"
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

pub async fn download_url(
    _client: &Client,
    url: &str,
    category_hint: Option<&str>,
) -> Result<Vec<u8>, MediaError> {
    let max_bytes = category_hint
        .map(max_bytes_for_category)
        .unwrap_or(MAX_FETCH_BYTES);

    let (response, _final_url) = timeout(DOWNLOAD_TIMEOUT, send_public_get(url))
        .await
        .map_err(|_| MediaError::Download("timeout".into()))?
        .map_err(|e| match e {
            SearchError::SsrfBlocked(msg) | SearchError::Invalid(msg) => {
                MediaError::SsrfBlocked(msg)
            }
            SearchError::Timeout => MediaError::Download("timeout".into()),
            other => MediaError::Download(other.to_string()),
        })?;

    if !response.status().is_success() {
        return Err(MediaError::Download(format!("HTTP {}", response.status())));
    }

    read_body_bounded(response, max_bytes).await
}

pub async fn download_and_validate(
    url: &str,
    category_hint: Option<&str>,
) -> Result<(Vec<u8>, String, super::models::MediaMetadata), MediaError> {
    // Client unused; send_public_get pins DNS per hop.
    let client = Client::new();
    let bytes = download_url(&client, url, category_hint).await?;
    let meta = validate_bytes(&bytes, category_hint)?;
    let hash = content_hash(&bytes);
    Ok((bytes, hash, meta))
}
