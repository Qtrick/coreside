//! Linkup's fixed-origin Search/Fetch client.
//!
//! This module is deliberately provider-local: callers receive Coreside's
//! normalized search/fetch types, never Linkup response objects or secrets.

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

const LINKUP_ORIGIN: &str = "https://api.linkup.so";
const LINKUP_SEARCH_URL: &str = "https://api.linkup.so/v1/search";
const LINKUP_FETCH_URL: &str = "https://api.linkup.so/v1/fetch";
pub const LINKUP_ENV_KEY: &str = "LINKUP_API_KEY";
const PROVIDER_ID: &str = "linkup";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

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

/// OS keyring wins over the developer-only environment fallback.
pub fn resolve_credentials() -> Credentials {
    resolve_credentials_with(
        || get_search_secret(&keyring_account()).ok(),
        || env::var(LINKUP_ENV_KEY).ok(),
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
        LINKUP_ORIGIN,
        crate::ai::platform::EndpointClass::FixedTrustedRemote,
        false,
        false,
        REQUEST_TIMEOUT,
    )
    .map(|(_, client)| client)
    .map_err(|_| SearchError::Provider("web retrieval is unavailable".into()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkupSearchRequest<'a> {
    q: &'a str,
    depth: &'static str,
    output_type: &'static str,
    max_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<&'a [String]>,
}

#[derive(Debug, Deserialize)]
struct LinkupResponse {
    #[serde(default)]
    results: Vec<LinkupResult>,
}

#[derive(Debug, Deserialize)]
struct LinkupResult {
    name: Option<String>,
    url: String,
    content: Option<String>,
    date: Option<String>,
    #[serde(rename = "type")]
    result_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkupFetchRequest<'a> {
    url: &'a str,
    extract_images: bool,
    include_raw_html: bool,
    render_js: bool,
}

#[derive(Debug, Deserialize)]
struct LinkupFetchResponse {
    content: Option<String>,
    #[serde(default)]
    results: Vec<LinkupFetchResult>,
}

#[derive(Debug, Deserialize)]
struct LinkupFetchResult {
    content: Option<String>,
}

async fn read_json_bounded(response: reqwest::Response) -> Result<LinkupResponse, SearchError> {
    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_RESPONSE_BYTES)
    {
        return Err(SearchError::Provider(
            "web retrieval response exceeded limits".into(),
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| SearchError::Provider("web retrieval failed".into()))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SearchError::Provider(
                "web retrieval response exceeded limits".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| SearchError::Provider("web retrieval returned an invalid response".into()))
}

async fn read_fetch_json_bounded(
    response: reqwest::Response,
) -> Result<LinkupFetchResponse, SearchError> {
    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_RESPONSE_BYTES)
    {
        return Err(SearchError::Provider(
            "web retrieval response exceeded limits".into(),
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| SearchError::Provider("web retrieval failed".into()))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SearchError::Provider(
                "web retrieval response exceeded limits".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| SearchError::Provider("web retrieval returned an invalid response".into()))
}

/// Normal discovery path: Linkup fast + raw search results only. It never asks
/// Linkup's Research endpoint or answer-generation modes to reason for Coreside.
pub async fn search(
    query: &str,
    count: usize,
    include_domains: Option<&[String]>,
) -> Result<WebSearchResponse, SearchError> {
    let credentials = resolve_credentials();
    let key = credentials.api_key.ok_or_else(|| {
        SearchError::NotConfigured(
            "Configure Linkup or set LINKUP_API_KEY for open-web search".into(),
        )
    })?;
    let request = LinkupSearchRequest {
        q: query,
        depth: "fast",
        output_type: "searchResults",
        max_results: count.clamp(1, 10),
        include_domains,
    };
    let response = timeout(
        REQUEST_TIMEOUT,
        client()?
            .post(LINKUP_SEARCH_URL)
            .bearer_auth(key)
            .json(&request)
            .send(),
    )
    .await
    .map_err(|_| SearchError::Timeout)?
    .map_err(|_| SearchError::Provider("web retrieval failed".into()))?;

    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return Err(SearchError::Credential(
                "Linkup credential was rejected".into(),
            ))
        }
        429 => return Err(SearchError::RateLimited),
        _ => return Err(SearchError::Provider("web retrieval is unavailable".into())),
    }
    let payload = read_json_bounded(response).await?;
    let results = payload
        .results
        .into_iter()
        .filter(|result| result.result_type.as_deref().unwrap_or("text") == "text")
        .filter_map(|result| {
            let url = validate_public_http_url(&result.url).ok()?;
            Some(WebSearchResult {
                id: format!("linkup-{}", Uuid::new_v4()),
                title: result
                    .name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| url.as_str().to_string()),
                url: url.to_string(),
                display_domain: url.host_str().map(str::to_string),
                snippet: result.content.map(|content| bound_text(&content, 400)),
                age: result.date,
                rank: 0,
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

pub async fn test_connection() -> Result<(), SearchError> {
    let _ = search("Coreside connectivity probe", 1, None).await?;
    Ok(())
}

/// Fetch a public page as Markdown. Raw HTML and JavaScript rendering are
/// explicitly disabled, and the returned text remains untrusted evidence.
pub async fn fetch_page(raw_url: &str) -> Result<FetchedWebPage, SearchError> {
    let safe_url = validate_public_http_url(raw_url)?;
    let credentials = resolve_credentials();
    let key = credentials.api_key.ok_or_else(|| {
        SearchError::NotConfigured("Configure Linkup or set LINKUP_API_KEY for web fetch".into())
    })?;
    let response = timeout(
        REQUEST_TIMEOUT,
        client()?
            .post(LINKUP_FETCH_URL)
            .bearer_auth(key)
            .json(&LinkupFetchRequest {
                url: safe_url.as_str(),
                extract_images: false,
                include_raw_html: false,
                render_js: false,
            })
            .send(),
    )
    .await
    .map_err(|_| SearchError::Timeout)?
    .map_err(|_| SearchError::Provider("web retrieval failed".into()))?;
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return Err(SearchError::Credential(
                "Linkup credential was rejected".into(),
            ))
        }
        429 => return Err(SearchError::RateLimited),
        _ => return Err(SearchError::Provider("web retrieval is unavailable".into())),
    }
    let payload = read_fetch_json_bounded(response).await?;
    let text = payload
        .content
        .or_else(|| {
            payload
                .results
                .into_iter()
                .find_map(|result| result.content)
        })
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| {
            SearchError::Provider("web retrieval returned no readable content".into())
        })?;
    Ok(FetchedWebPage {
        url: safe_url.to_string(),
        final_url: safe_url.to_string(),
        title: None,
        byte_size: text.len(),
        text: bound_text(&text, MAX_RESPONSE_BYTES),
        content_type: Some("text/markdown".into()),
    })
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
        assert_eq!(keyring_account(), "coreside.search.linkup");
    }

    #[test]
    fn fetch_response_never_requires_raw_html() {
        let parsed: LinkupFetchResponse =
            serde_json::from_str(r##"{"results":[{"content":"# Safe markdown"}]}"##)
                .expect("valid provider response");
        assert_eq!(
            parsed.results[0].content.as_deref(),
            Some("# Safe markdown")
        );
        assert!(parsed.content.is_none());
    }
}
