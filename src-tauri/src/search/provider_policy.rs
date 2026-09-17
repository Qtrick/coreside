//! Network safety and provider-side fetch policy.
//!
//! Enforces clear security boundaries between:
//! 1. Coreside-owned fetch (strict SSRF + socket DNS pinning)
//! 2. Local sidecar crawl (Crawl4AI sandbox boundaries)
//! 3. Remote provider fetch (Firecrawl / Linkup - never delegate sensitive/intranet targets)

use serde::{Deserialize, Serialize};
use url::Url;

use super::errors::SearchError;
use super::safety::{
    validate_and_pin_public_http_url, validate_public_http_url, validate_public_url_structure,
    PinnedPublicUrl,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchChannel {
    /// Local client with direct socket address pinning (prevents DNS rebinding).
    CoresideOwnedPinned,
    /// Local Crawl4AI Python sidecar process.
    LocalSidecar,
    /// Remote third-party provider (Firecrawl / Linkup).
    RemoteProvider,
}

#[derive(Debug, Clone)]
pub enum AuthorizedFetch {
    /// Coreside-owned fetch with verified and pinned IP addresses.
    Pinned(PinnedPublicUrl),
    /// URL safe for delegation to an external provider or sidecar.
    Delegated { url: Url, channel: FetchChannel },
}

/// Verify that a URL is eligible for third-party provider delegation (Firecrawl / Linkup).
/// Remote providers must NEVER receive private, intranet, localhost, or reserved destinations.
/// Note: Coreside validates URL structure and private ranges, but remote providers perform
/// their own resolution in their cloud environments.
pub fn authorize_remote_provider_fetch(
    raw_url: &str,
    provider_name: &str,
) -> Result<Url, SearchError> {
    let url = validate_public_url_structure(raw_url)?;

    let host = url.host_str().ok_or_else(|| {
        SearchError::SsrfBlocked(format!("missing host for {provider_name} delegation"))
    })?;

    // Additional defense-in-depth against delegating intranet/internal names
    let host_lower = host.to_lowercase();
    if host_lower.contains("internal")
        || host_lower.contains("corp")
        || host_lower.contains("intranet")
        || host_lower.contains("private")
        || host_lower.ends_with(".lan")
        || host_lower.ends_with(".home")
    {
        return Err(SearchError::SsrfBlocked(format!(
            "refusing to delegate internal/intranet host '{host}' to remote provider {provider_name}"
        )));
    }

    Ok(url)
}

/// Authorize a URL for Crawl4AI sidecar execution.
/// Enforces that the target is a publicly addressable HTTP(S) endpoint.
pub fn authorize_sidecar_crawl(raw_url: &str) -> Result<Url, SearchError> {
    let url = validate_public_url_structure(raw_url)?;
    Ok(url)
}

/// Authorize a URL for Coreside-owned fetching with DNS pinning.
pub fn authorize_coreside_pinned_fetch(raw_url: &str) -> Result<PinnedPublicUrl, SearchError> {
    validate_and_pin_public_http_url(raw_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_web_delegation() {
        assert!(authorize_remote_provider_fetch("https://example.com/page", "firecrawl").is_ok());
        assert!(authorize_sidecar_crawl("https://rust-lang.org").is_ok());
        assert!(authorize_coreside_pinned_fetch("https://1.1.1.1/dns-query").is_ok());
    }

    #[test]
    fn blocks_private_and_localhost_delegation() {
        assert!(
            authorize_remote_provider_fetch("http://localhost:8080/secret", "firecrawl").is_err()
        );
        assert!(authorize_remote_provider_fetch("http://127.0.0.1/admin", "firecrawl").is_err());
        assert!(authorize_remote_provider_fetch("http://10.0.0.1/", "firecrawl").is_err());
        assert!(authorize_remote_provider_fetch(
            "http://169.254.169.254/latest/meta-data",
            "firecrawl"
        )
        .is_err());
        assert!(authorize_sidecar_crawl("http://192.168.1.1/router").is_err());
    }

    #[test]
    fn blocks_intranet_and_internal_hostnames() {
        assert!(
            authorize_remote_provider_fetch("https://corp.internal/dashboard", "linkup").is_err()
        );
        assert!(authorize_remote_provider_fetch("https://router.lan/", "firecrawl").is_err());
    }
}
