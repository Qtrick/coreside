use reqwest::Client;
use tokio::time::{timeout, Duration};

use crate::search::{build_http_client, validate_public_http_url, MAX_FETCH_BYTES};

use super::errors::MediaError;
use super::limits::max_bytes_for_category;
use super::validation::{content_hash, validate_bytes};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

pub async fn download_url(
    client: &Client,
    url: &str,
    category_hint: Option<&str>,
) -> Result<Vec<u8>, MediaError> {
    validate_public_http_url(url).map_err(|e| MediaError::SsrfBlocked(e.to_string()))?;

    let max_bytes = category_hint
        .map(max_bytes_for_category)
        .unwrap_or(MAX_FETCH_BYTES);

    let response = timeout(DOWNLOAD_TIMEOUT, client.get(url).send())
        .await
        .map_err(|_| MediaError::Download("timeout".into()))?
        .map_err(|e| MediaError::Download(e.to_string()))?;

    if !response.status().is_success() {
        return Err(MediaError::Download(format!("HTTP {}", response.status())));
    }

    let final_url = response.url().to_string();
    validate_public_http_url(&final_url).map_err(|e| MediaError::SsrfBlocked(e.to_string()))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| MediaError::Download(e.to_string()))?;

    if bytes.len() > max_bytes {
        return Err(MediaError::Invalid(format!(
            "download exceeds {max_bytes} bytes"
        )));
    }

    Ok(bytes.to_vec())
}

pub async fn download_and_validate(
    url: &str,
    category_hint: Option<&str>,
) -> Result<(Vec<u8>, String, super::models::MediaMetadata), MediaError> {
    let client = build_http_client().map_err(|e| MediaError::Download(e.to_string()))?;
    let bytes = download_url(&client, url, category_hint).await?;
    let meta = validate_bytes(&bytes, category_hint)?;
    let hash = content_hash(&bytes);
    Ok((bytes, hash, meta))
}
