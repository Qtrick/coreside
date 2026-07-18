//! SSRF-safe URL validation for outbound fetch.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;

use url::Url;

use super::errors::SearchError;

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "localhost.localdomain",
    "127.0.0.1",
    "::1",
    "0.0.0.0",
    "[::1]",
    "metadata.google.internal",
    "metadata.goog",
];

/// Reject URLs that could reach private networks or local resources.
///
/// Literal private/reserved IPs are blocked immediately. Hostnames are DNS-resolved
/// and every returned address must be public (fail closed on resolution failure).
pub fn validate_public_http_url(raw: &str) -> Result<Url, SearchError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SearchError::Invalid("URL is required".into()));
    }

    let url = Url::parse(trimmed).map_err(|e| SearchError::SsrfBlocked(e.to_string()))?;

    let scheme = url.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(SearchError::SsrfBlocked(format!(
            "unsupported protocol: {scheme}"
        )));
    }

    if url.username() != "" || url.password().is_some() {
        return Err(SearchError::SsrfBlocked(
            "embedded credentials are not allowed".into(),
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| SearchError::SsrfBlocked("missing host".into()))?;

    let host_lower = host.to_lowercase();
    if BLOCKED_HOSTS.iter().any(|h| host_lower == *h) {
        return Err(SearchError::SsrfBlocked(format!("blocked host: {host}")));
    }

    if host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        return Err(SearchError::SsrfBlocked(format!("blocked host: {host}")));
    }

    if let Ok(ip) = IpAddr::from_str(host) {
        if is_private_or_reserved(ip) {
            return Err(SearchError::SsrfBlocked(format!("private IP: {ip}")));
        }
        return Ok(url);
    }

    // Bracketed IPv6 literals (url::Host may keep brackets in host_str).
    if host.starts_with('[') && host.ends_with(']') {
        let inner = &host[1..host.len() - 1];
        if let Ok(ip) = IpAddr::from_str(inner) {
            if is_private_or_reserved(ip) {
                return Err(SearchError::SsrfBlocked(format!("private IP: {ip}")));
            }
            return Ok(url);
        }
    }

    resolve_public_host(&host_lower)?;
    Ok(url)
}

fn resolve_public_host(host: &str) -> Result<(), SearchError> {
    let addrs = (host, 80)
        .to_socket_addrs()
        .map_err(|e| SearchError::SsrfBlocked(format!("DNS resolution failed for {host}: {e}")))?;

    let mut saw_any = false;
    for addr in addrs {
        saw_any = true;
        let ip = addr.ip();
        if is_private_or_reserved(ip) {
            return Err(SearchError::SsrfBlocked(format!(
                "host {host} resolves to private IP: {ip}"
            )));
        }
    }
    if !saw_any {
        return Err(SearchError::SsrfBlocked(format!(
            "DNS returned no addresses for {host}"
        )));
    }
    Ok(())
}

fn is_private_or_reserved(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.octets()[0] == 0
        || matches!(ip.octets(), [127, _, _, _])
        || matches!(ip.octets(), [10, _, _, _])
        || matches!(ip.octets(), [172, b, _, _] if (16..=31).contains(&b))
        || matches!(ip.octets(), [192, 168, _, _])
        || matches!(ip.octets(), [169, 254, _, _])
        || matches!(ip.octets(), [100, 64..=127, _, _])
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_private_v4(v4);
    }
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.segments()[0] & 0xfe00 == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_file_protocol() {
        assert!(validate_public_http_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn rejects_localhost() {
        assert!(validate_public_http_url("http://localhost/").is_err());
        assert!(validate_public_http_url("http://127.0.0.1/").is_err());
        assert!(validate_public_http_url("http://[::1]/").is_err());
    }

    #[test]
    fn rejects_private_ranges() {
        assert!(validate_public_http_url("http://10.0.0.1/").is_err());
        assert!(validate_public_http_url("http://172.16.0.1/").is_err());
        assert!(validate_public_http_url("http://192.168.1.1/").is_err());
        assert!(validate_public_http_url("http://169.254.1.1/").is_err());
    }

    #[test]
    fn allows_public_https_ip_literal() {
        // Use a public IP literal so the unit test does not depend on DNS/network.
        assert!(validate_public_http_url("https://1.1.1.1/").is_ok());
        assert!(validate_public_http_url("https://8.8.8.8/dns").is_ok());
    }

    #[test]
    fn rejects_internal_and_metadata_hosts() {
        assert!(validate_public_http_url("http://metadata.google.internal/").is_err());
        assert!(validate_public_http_url("http://foo.internal/bar").is_err());
    }

    #[test]
    fn rejects_public_ipv4_literals_that_are_private() {
        // Covered by rejects_private_ranges; ensure early-return path stays fail-closed.
        assert!(validate_public_http_url("https://169.254.169.254/latest").is_err());
    }
}
