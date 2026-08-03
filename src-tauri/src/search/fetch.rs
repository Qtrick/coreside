use std::env;
use std::time::Duration;

use reqwest::Client;
use tokio::time::timeout;

use super::errors::SearchError;
use super::models::FetchedWebPage;
use super::normalization::{bound_text, extract_text_from_html};
use super::safety::{validate_and_pin_public_http_url, validate_public_http_url, PinnedPublicUrl};

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_REDIRECTS: usize = 3;
pub const MAX_FETCH_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_EXTRACTED_TEXT_CHARS: usize = 32_000;

const SEARCH_KEYRING_SERVICE: &str = "coreside.search";
const BRAVE_ACCOUNT: &str = "brave";

pub fn search_keyring_account(provider: &str) -> String {
    format!("coreside.search.{provider}")
}

/// Resolve legacy search API key from keyring / env (optional cleanup only; not required).
pub fn resolve_search_api_key(provider: &str) -> Result<Option<String>, SearchError> {
    let account = search_keyring_account(provider);
    if let Ok(key) = get_search_secret(&account) {
        if !key.trim().is_empty() {
            return Ok(Some(key.trim().to_string()));
        }
    }
    Ok(resolve_search_env_key())
}

pub fn resolve_search_env_key() -> Option<String> {
    for var in ["BRAVE_SEARCH_API_KEY", "SEARCH_API_KEY"] {
        if let Ok(v) = env::var(var) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn has_search_key(provider: &str) -> bool {
    resolve_search_api_key(provider).ok().flatten().is_some()
}

pub fn set_search_secret(account: &str, secret: &str) -> Result<(), SearchError> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(SearchError::Credential("API key cannot be empty".into()));
    }
    keyring::Entry::new(SEARCH_KEYRING_SERVICE, account)
        .map_err(|e| SearchError::Credential(e.to_string()))?
        .set_password(secret)
        .map_err(|e| SearchError::Credential(e.to_string()))
}

pub fn get_search_secret(account: &str) -> Result<String, SearchError> {
    keyring::Entry::new(SEARCH_KEYRING_SERVICE, account)
        .map_err(|e| SearchError::Credential(e.to_string()))?
        .get_password()
        .map_err(|e| match e {
            keyring::Error::NoEntry => SearchError::Credential("not found".into()),
            other => SearchError::Credential(other.to_string()),
        })
}

pub fn delete_search_secret(account: &str) -> Result<(), SearchError> {
    match keyring::Entry::new(SEARCH_KEYRING_SERVICE, account)
        .map_err(|e| SearchError::Credential(e.to_string()))?
        .delete_credential()
    {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SearchError::Credential(e.to_string())),
    }
}

pub fn brave_account() -> &'static str {
    BRAVE_ACCOUNT
}

fn pinned_client(pinned: &PinnedPublicUrl) -> Result<Client, SearchError> {
    let host = pinned.host_for_resolve()?.to_string();
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(FETCH_TIMEOUT)
        .no_proxy()
        .resolve_to_addrs(&host, &pinned.addrs)
        .user_agent("Coreside/0.1 (+https://coreside.local)")
        .build()
        .map_err(|e| SearchError::Fetch(e.to_string()))
}

/// Shared public-web client for callers that only need timeouts/no_proxy.
/// Prefer [`send_public_get`] for URL fetches so DNS stays pinned per hop.
pub fn build_http_client() -> Result<Client, SearchError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(FETCH_TIMEOUT)
        .no_proxy()
        .user_agent("Coreside/0.1 (+https://coreside.local)")
        .build()
        .map_err(|e| SearchError::Fetch(e.to_string()))
}

/// GET with DNS pin + manual redirect hop validation.
pub async fn send_public_get(raw_url: &str) -> Result<(reqwest::Response, String), SearchError> {
    let mut pinned = validate_and_pin_public_http_url(raw_url)?;
    let mut hops = 0usize;
    loop {
        let client = pinned_client(&pinned)?;
        let response = timeout(FETCH_TIMEOUT, client.get(pinned.url.clone()).send())
            .await
            .map_err(|_| SearchError::Timeout)?
            .map_err(|e| SearchError::Fetch(e.to_string()))?;

        let status = response.status();
        if status.is_redirection() {
            if hops >= MAX_REDIRECTS {
                return Err(SearchError::SsrfBlocked("too many redirects".into()));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| SearchError::SsrfBlocked("redirect missing Location".into()))?;
            let next = pinned
                .url
                .join(location)
                .map_err(|e| SearchError::SsrfBlocked(format!("bad redirect: {e}")))?;
            pinned = validate_and_pin_public_http_url(next.as_str())?;
            hops += 1;
            continue;
        }

        let final_url = pinned.url.to_string();
        return Ok((response, final_url));
    }
}

/// Stream response body with a hard decompressed-byte budget.
async fn read_body_bounded(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, SearchError> {
    use futures_util::StreamExt;

    if let Some(len) = response.content_length() {
        if len as usize > max_bytes {
            return Err(SearchError::Fetch(format!(
                "declared Content-Length {len} exceeds {max_bytes} bytes"
            )));
        }
    }

    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| SearchError::Fetch(e.to_string()))?;
        if out.len().saturating_add(chunk.len()) > max_bytes {
            return Err(SearchError::Fetch(format!(
                "response exceeds {max_bytes} bytes"
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// SSRF-safe webpage fetch with size limits and HTML text extraction.
pub async fn fetch_web_page(client: &Client, raw_url: &str) -> Result<FetchedWebPage, SearchError> {
    let _ = client; // callers may still pass a shared client; hops use pinned clients.
    let _ = validate_public_http_url(raw_url)?;
    let (response, final_url) = send_public_get(raw_url).await?;

    if response.status().as_u16() == 429 {
        return Err(SearchError::RateLimited);
    }
    if !response.status().is_success() {
        return Err(SearchError::Fetch(format!("HTTP {}", response.status())));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let bytes = read_body_bounded(response, MAX_FETCH_BYTES).await?;

    let body = String::from_utf8_lossy(&bytes);
    let is_html = content_type
        .as_deref()
        .map(|ct| ct.contains("html"))
        .unwrap_or_else(|| body.trim_start().starts_with('<'));

    let text = if is_html {
        extract_text_from_html(&body)
    } else {
        bound_text(body.trim(), MAX_EXTRACTED_TEXT_CHARS)
    };

    let title = if is_html { extract_title(&body) } else { None };

    Ok(FetchedWebPage {
        url: raw_url.trim().to_string(),
        final_url,
        title,
        text: bound_text(&text, MAX_EXTRACTED_TEXT_CHARS),
        content_type,
        byte_size: bytes.len(),
    })
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let after = &html[start..];
    let gt = after.find('>')? + 1;
    let rest = &after[gt..];
    let end = rest.to_lowercase().find("</title>")?;
    let title = rest[..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(bound_text(title, 240))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_key_resolution_order() {
        std::env::set_var("BRAVE_SEARCH_API_KEY", "brave-test");
        std::env::set_var("SEARCH_API_KEY", "generic");
        assert_eq!(resolve_search_env_key().as_deref(), Some("brave-test"));
        std::env::remove_var("BRAVE_SEARCH_API_KEY");
        assert_eq!(resolve_search_env_key().as_deref(), Some("generic"));
        std::env::remove_var("SEARCH_API_KEY");
    }
}
