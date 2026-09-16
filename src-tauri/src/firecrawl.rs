//! Firecrawl search/scrape client for difficult pages, JS-heavy sites, and
//! deep extraction.  Provider-local: callers receive Coreside's normalized
//! types, never Firecape response objects or secrets.

use std::env;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use uuid::Uuid;

use crate::search::{
    bound_text, normalize_web_results, search_keyring_account, validate_public_http_url,
    FetchedWebPage, WebSearchResponse, WebSearchResult,
};
use crate::search::{get_search_secret, SearchError};

const FIRECRAWL_ORIGIN: &str = "https://api.firecrawl.dev";
const FIRECRAWL_SEARCH_URL: &str = "https://api.firecrawl.dev/v2/search";
const FIRECRAWL_SCRAPE_URL: &str = "https://api.firecrawl.dev/v2/scrape";
pub const FIRECRAWL_ENV_KEY: &str = "FIRECRAWL_API_KEY";
const PROVIDER_ID: &str = "firecrawl";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    Keyring,
    Env,
    None,
}

impl CredentialSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::Env => "env",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub api_key: Option<String>,
    pub source: CredentialSource,
}

pub fn keyring_account() -> String {
    search_keyring_account(PROVIDER_ID)
}

pub fn resolve_credentials() -> Credentials {
    resolve_credentials_with(
        || get_search_secret(&keyring_account()).ok(),
        || env::var(FIRECRAWL_ENV_KEY).ok(),
    )
}

pub fn resolve_credentials_with<K, E>(keyring: K, env_key: E) -> Credentials
where
    K: FnOnce() -> Option<String>,
    E: FnOnce() -> Option<String>,
{
    for (candidate, source) in [
        (keyring(), CredentialSource::Keyring),
        (env_key(), CredentialSource::Env),
    ] {
        if let Some(key) = candidate {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Credentials {
                    api_key: Some(key),
                    source,
                };
            }
        }
    }
    Credentials {
        api_key: None,
        source: CredentialSource::None,
    }
}

pub fn has_key() -> bool {
    resolve_credentials().api_key.is_some()
}

fn client() -> Result<Client, SearchError> {
    crate::ai::platform::validate_and_build_credential_client(
        FIRECRAWL_ORIGIN,
        crate::ai::platform::EndpointClass::FixedTrustedRemote,
        false,
        false,
        REQUEST_TIMEOUT,
    )
    .map(|(_, client)| client)
    .map_err(|_| SearchError::Provider("firecrawl is unavailable".into()))
}

// ── Search ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlSearchRequest<'a> {
    query: &'a str,
    limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    scrape_options: Option<FirecrawlScrapeOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlScrapeOptions {
    formats: Vec<&'static str>,
    only_main_content: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlSearchResponse {
    success: bool,
    #[serde(default)]
    data: Vec<FirecrawlSearchResult>,
    #[serde(default)]
    credits_used: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlSearchResult {
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    metadata: Option<FirecrawlMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlMetadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "sourceURL")]
    source_url: Option<String>,
}

// ── Scrape ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirecrawlScrapeRequest {
    url: String,
    formats: Vec<&'static str>,
    only_main_content: bool,
}

#[derive(Debug, Deserialize)]
struct FirecrawlScrapeResponse {
    success: bool,
    #[serde(default)]
    data: Option<FirecrawlScrapeData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlScrapeData {
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    html: Option<String>,
    #[serde(default)]
    metadata: Option<FirecrawlMetadata>,
}

// ── Helpers ─────────────────────────────────────────────────────────────

async fn read_json_bounded<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, SearchError> {
    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_RESPONSE_BYTES)
    {
        return Err(SearchError::Provider(
            "firecrawl response exceeded limits".into(),
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| SearchError::Provider("firecrawl fetch failed".into()))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SearchError::Provider(
                "firecrawl response exceeded limits".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| SearchError::Provider("firecrawl returned an invalid response".into()))
}

fn map_status(status: u16) -> Result<(), SearchError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(SearchError::Credential(
            "Firecrawl credential was rejected".into(),
        )),
        402 => Err(SearchError::Provider("Firecrawl credits exhausted".into())),
        429 => Err(SearchError::RateLimited),
        _ => Err(SearchError::Provider("firecrawl is unavailable".into())),
    }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Search the web via Firecrawl.  Returns SSRF-validated, normalized results.
/// Firecrawl search returns markdown snippets; we use the description field as
/// the snippet for consistency with other providers.
pub async fn search(
    query: &str,
    count: usize,
    include_domains: Option<&[String]>,
) -> Result<WebSearchResponse, SearchError> {
    let credentials = resolve_credentials();
    let key = credentials.api_key.ok_or_else(|| {
        SearchError::NotConfigured(
            "Configure Firecrawl or set FIRECRAWL_API_KEY for deep web research".into(),
        )
    })?;
    let clamped = count.clamp(1, 20);
    let request = FirecrawlSearchRequest {
        query,
        limit: clamped,
        scrape_options: Some(FirecrawlScrapeOptions {
            formats: vec!["markdown"],
            only_main_content: true,
        }),
    };
    let mut builder = client()?
        .post(FIRECRAWL_SEARCH_URL)
        .bearer_auth(&key)
        .json(&request);
    if let Some(domains) = include_domains {
        // Firecrawl includeDomains is top-level, not nested in scrapeOptions.
        // We pass it via a JSON merge — but since the struct doesn't have the
        // field, we add it manually for correctness.
        let mut body = serde_json::to_value(&request)
            .map_err(|_| SearchError::Provider("firecrawl request serialization failed".into()))?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "includeDomains".into(),
                serde_json::to_value(domains).map_err(|_| {
                    SearchError::Provider("firecrawl domain serialization failed".into())
                })?,
            );
        }
        builder = builder.json(&body);
    }
    let response = timeout(REQUEST_TIMEOUT, builder.send())
        .await
        .map_err(|_| SearchError::Timeout)?
        .map_err(|_| SearchError::Provider("firecrawl request failed".into()))?;
    map_status(response.status().as_u16())?;
    let payload = read_json_bounded::<FirecrawlSearchResponse>(response).await?;
    if !payload.success {
        return Err(SearchError::Provider(
            payload
                .error
                .unwrap_or_else(|| "firecrawl search failed".into()),
        ));
    }
    let results = payload
        .data
        .into_iter()
        .filter_map(|result| {
            let url = validate_public_http_url(&result.url).ok()?;
            let title = result
                .title
                .or(result.metadata.as_ref().and_then(|m| m.title.clone()))
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| url.as_str().to_string());
            let snippet = result
                .description
                .or_else(|| result.markdown.as_ref().map(|md| bound_text(md, 400)))
                .filter(|s| !s.trim().is_empty());
            let content_preview = result.markdown.as_ref().map(|md| bound_text(md, 2000));
            Some(WebSearchResult {
                id: format!("firecrawl-{}", Uuid::new_v4()),
                title,
                url: url.to_string(),
                display_domain: url.host_str().map(str::to_string),
                snippet,
                age: None,
                rank: 0,
                provider: Some(PROVIDER_ID.into()),
                canonical_url: Some(url.to_string()),
                content: content_preview,
                highlights: None,
                fetched_at: Some(crate::db::now_rfc3339()),
                retrieval_method: Some("firecrawl_search".into()),
                score: None,
            })
        })
        .collect::<Vec<_>>();
    let mut results = normalize_web_results(results);
    for (index, result) in results.iter_mut().enumerate() {
        result.rank = index + 1;
    }
    Ok(WebSearchResponse {
        query: query.to_string(),
        provider: PROVIDER_ID.into(),
        results,
        session_id: None,
        notice: None,
    })
}

/// Scrape a single URL and return clean Markdown.  Used for deep extraction
/// from pages where provider snippets are insufficient (JS-heavy, PDF, etc.).
pub async fn scrape(raw_url: &str) -> Result<FetchedWebPage, SearchError> {
    let safe_url = validate_public_http_url(raw_url)?;
    let credentials = resolve_credentials();
    let key = credentials.api_key.ok_or_else(|| {
        SearchError::NotConfigured("Configure Firecrawl or set FIRECRAWL_API_KEY for scrape".into())
    })?;
    let request = FirecrawlScrapeRequest {
        url: safe_url.to_string(),
        formats: vec!["markdown"],
        only_main_content: true,
    };
    let response = timeout(
        REQUEST_TIMEOUT,
        client()?
            .post(FIRECRAWL_SCRAPE_URL)
            .bearer_auth(&key)
            .json(&request)
            .send(),
    )
    .await
    .map_err(|_| SearchError::Timeout)?
    .map_err(|_| SearchError::Provider("firecrawl scrape failed".into()))?;
    map_status(response.status().as_u16())?;
    let payload = read_json_bounded::<FirecrawlScrapeResponse>(response).await?;
    if !payload.success {
        return Err(SearchError::Provider(
            payload
                .error
                .unwrap_or_else(|| "firecrawl scrape failed".into()),
        ));
    }
    let data = payload
        .data
        .ok_or_else(|| SearchError::Provider("firecrawl scrape returned no data".into()))?;
    let text = data
        .markdown
        .filter(|md| !md.trim().is_empty())
        .ok_or_else(|| {
            SearchError::Provider("firecrawl scrape returned no readable content".into())
        })?;
    let title = data
        .metadata
        .as_ref()
        .and_then(|m| m.title.clone())
        .filter(|t| !t.trim().is_empty());
    let final_url = data
        .metadata
        .as_ref()
        .and_then(|m| m.source_url.clone())
        .unwrap_or_else(|| safe_url.to_string());
    Ok(FetchedWebPage {
        url: safe_url.to_string(),
        final_url,
        title,
        byte_size: text.len(),
        text: bound_text(&text, MAX_MARKDOWN_BYTES),
        content_type: Some("text/markdown".into()),
    })
}

pub async fn test_connection() -> Result<(), SearchError> {
    let _ = search("Coreside connectivity probe", 1, None).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_beats_env() {
        let credentials =
            resolve_credentials_with(|| Some(" keyring ".into()), || Some("env".into()));
        assert_eq!(credentials.api_key.as_deref(), Some("keyring"));
        assert_eq!(credentials.source, CredentialSource::Keyring);
    }

    #[test]
    fn env_is_developer_fallback() {
        let credentials = resolve_credentials_with(|| None, || Some(" env ".into()));
        assert_eq!(credentials.api_key.as_deref(), Some("env"));
        assert_eq!(credentials.source.as_str(), "env");
        assert_eq!(keyring_account(), "coreside.search.firecrawl");
    }

    #[test]
    fn scrape_response_parses() {
        let json = r#"{
            "success": true,
            "data": {
                "markdown": "Hello World",
                "metadata": { "title": "Hello", "sourceURL": "https://example.com" }
            }
        }"#;
        let parsed: FirecrawlScrapeResponse =
            serde_json::from_str(json).expect("valid scrape response");
        assert!(parsed.success);
        let data = parsed.data.unwrap();
        assert_eq!(data.markdown.as_deref(), Some("Hello World"));
        assert_eq!(
            data.metadata.as_ref().and_then(|m| m.source_url.as_deref()),
            Some("https://example.com")
        );
    }

    #[test]
    fn search_response_parses() {
        let json = r#"{
            "success": true,
            "data": [
                { "url": "https://example.com", "title": "Example", "description": "An example" }
            ],
            "creditsUsed": 2
        }"#;
        let parsed: FirecrawlSearchResponse =
            serde_json::from_str(json).expect("valid search response");
        assert!(parsed.success);
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.credits_used, Some(2));
    }
}
