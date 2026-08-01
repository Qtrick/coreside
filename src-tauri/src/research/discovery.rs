//! Honest discovery seeding: URL / domain / needs-seed (no fabricated SERP).

use url::Url;

pub const NEEDS_SEED_MESSAGE: &str =
    "Local research needs a URL or site to start from. Provide a URL (https://…) or a domain (example.com). Coreside does not invent global search results.";

/// Shown for free-text open-web queries when Exa is not configured.
pub const NEEDS_EXA_OR_SEED_MESSAGE: &str =
    "Open-web search needs Exa configured, or provide a URL (https://…) / domain (example.com) for local Crawl4AI research.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoverySeed {
    Url(String),
    Domain(String),
    NeedsSeed,
}

/// Classify a research query into a crawlable seed.
///
/// Prefer explicit `domain` when provided; otherwise treat URL-shaped queries as direct
/// crawl targets. Free-text without a seed returns `NeedsSeed` (honest empty).
pub fn classify_discovery_seed(query: &str, domain: Option<&str>) -> DiscoverySeed {
    if let Some(d) = domain.map(str::trim).filter(|s| !s.is_empty()) {
        return DiscoverySeed::Domain(normalize_domain(d));
    }

    let q = query.trim();
    if q.is_empty() {
        return DiscoverySeed::NeedsSeed;
    }

    if let Some(url) = look_like_url(q) {
        return DiscoverySeed::Url(url);
    }

    if looks_like_bare_domain(q) {
        return DiscoverySeed::Domain(normalize_domain(q));
    }

    DiscoverySeed::NeedsSeed
}

fn look_like_url(raw: &str) -> Option<String> {
    let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.starts_with("www.") {
        format!("https://{raw}")
    } else {
        return None;
    };
    let parsed = Url::parse(&candidate).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    parsed.host_str()?;
    Some(candidate)
}

fn looks_like_bare_domain(raw: &str) -> bool {
    if raw.contains(' ') || raw.contains('/') {
        return false;
    }
    let host = raw.strip_prefix("www.").unwrap_or(raw);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && parts.last().map(|t| t.len() >= 2).unwrap_or(false)
}

fn normalize_domain(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if let Ok(url) = Url::parse(trimmed) {
        if let Some(host) = url.host_str() {
            return host.trim_start_matches("www.").to_lowercase();
        }
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .trim_start_matches("www.")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_query_becomes_url_seed() {
        assert_eq!(
            classify_discovery_seed("https://docs.rust-lang.org/book/", None),
            DiscoverySeed::Url("https://docs.rust-lang.org/book/".into())
        );
    }

    #[test]
    fn bare_domain_becomes_domain_seed() {
        assert_eq!(
            classify_discovery_seed("example.com", None),
            DiscoverySeed::Domain("example.com".into())
        );
    }

    #[test]
    fn free_text_needs_seed() {
        assert_eq!(
            classify_discovery_seed("best rust web frameworks 2026", None),
            DiscoverySeed::NeedsSeed
        );
    }

    #[test]
    fn explicit_domain_wins() {
        assert_eq!(
            classify_discovery_seed("anything", Some("docs.python.org")),
            DiscoverySeed::Domain("docs.python.org".into())
        );
    }
}
