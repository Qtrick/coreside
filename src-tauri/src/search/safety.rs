//! SSRF-safe URL validation with DNS-to-connection address pinning.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use url::Url;

use super::errors::SearchError;

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "localhost.localdomain",
    "ip6-localhost",
    "ip6-loopback",
    "127.0.0.1",
    "::1",
    "0.0.0.0",
    "[::1]",
    "metadata.google.internal",
    "metadata.goog",
    "169.254.169.254",
    "instance-data",
    "instance-data.ec2.internal",
];

/// Validated public URL plus the socket addresses the HTTP client must use.
#[derive(Debug, Clone)]
pub struct PinnedPublicUrl {
    pub url: Url,
    pub addrs: Vec<SocketAddr>,
}

impl PinnedPublicUrl {
    pub fn host_for_resolve(&self) -> Result<&str, SearchError> {
        self.url
            .host_str()
            .ok_or_else(|| SearchError::SsrfBlocked("missing host".into()))
    }
}

/// Reject URLs that could reach private networks and pin validated addresses.
pub fn validate_public_http_url(raw: &str) -> Result<Url, SearchError> {
    Ok(validate_and_pin_public_http_url(raw)?.url)
}

/// Validate scheme, port, credentials, blocked hosts, and IP literals without DNS resolution.
/// Used for provider-side delegation policies where local DNS does not control remote fetching.
pub fn validate_public_url_structure(raw: &str) -> Result<Url, SearchError> {
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
        || host_lower.ends_with(".lan")
        || host_lower.ends_with(".corp")
        || host_lower.ends_with(".home")
        || host_lower.ends_with(".arpa")
        || host_lower.ends_with(".onion")
        || host_lower.ends_with(".intranet")
        || host_lower.ends_with(".private")
    {
        return Err(SearchError::SsrfBlocked(format!("blocked host: {host}")));
    }

    let port = url
        .port_or_known_default()
        .ok_or_else(|| SearchError::SsrfBlocked("missing port".into()))?;

    // Public research/media: only standard web ports unless explicitly expanded later.
    if port != 80 && port != 443 {
        return Err(SearchError::SsrfBlocked(format!(
            "public-web port not allowed: {port}"
        )));
    }

    if let Ok(ip) = IpAddr::from_str(host) {
        if is_private_or_reserved(ip) {
            return Err(SearchError::SsrfBlocked(format!("private IP: {ip}")));
        }
    }

    if host.starts_with('[') && host.ends_with(']') {
        let inner = &host[1..host.len() - 1];
        if let Ok(ip) = IpAddr::from_str(inner) {
            if is_private_or_reserved(ip) {
                return Err(SearchError::SsrfBlocked(format!("private IP: {ip}")));
            }
        }
    }

    Ok(url)
}

/// Validate scheme/host policy and resolve DNS once for connection pinning.
pub fn validate_and_pin_public_http_url(raw: &str) -> Result<PinnedPublicUrl, SearchError> {
    let url = validate_public_url_structure(raw)?;
    let host = url.host_str().unwrap();
    let port = url.port_or_known_default().unwrap();

    if let Ok(ip) = IpAddr::from_str(host) {
        return Ok(PinnedPublicUrl {
            url,
            addrs: vec![SocketAddr::new(ip, port)],
        });
    }

    if host.starts_with('[') && host.ends_with(']') {
        let inner = &host[1..host.len() - 1];
        if let Ok(ip) = IpAddr::from_str(inner) {
            return Ok(PinnedPublicUrl {
                url,
                addrs: vec![SocketAddr::new(ip, port)],
            });
        }
    }

    let addrs = resolve_public_host_addrs(&host.to_lowercase(), port)?;
    Ok(PinnedPublicUrl { url, addrs })
}

fn resolve_public_host_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, SearchError> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| SearchError::SsrfBlocked(format!("DNS resolution failed for {host}: {e}")))?;

    let mut out = Vec::new();
    for addr in addrs {
        let ip = addr.ip();
        if is_private_or_reserved(ip) {
            return Err(SearchError::SsrfBlocked(format!(
                "host {host} resolves to private IP: {ip}"
            )));
        }
        out.push(SocketAddr::new(ip, port));
    }
    if out.is_empty() {
        return Err(SearchError::SsrfBlocked(format!(
            "DNS returned no addresses for {host}"
        )));
    }
    Ok(out)
}

fn is_private_or_reserved(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || octets[0] == 0
        || octets[0] >= 240 // Class E / Reserved (RFC 1112)
        || matches!(octets, [127, _, _, _])
        || matches!(octets, [10, _, _, _])
        || matches!(octets, [172, b, _, _] if (16..=31).contains(&b))
        || matches!(octets, [192, 168, _, _])
        || matches!(octets, [169, 254, _, _])
        || matches!(octets, [100, 64..=127, _, _]) // CGNAT (RFC 6598)
        || matches!(octets, [198, 18..=19, _, _]) // Benchmark testing (RFC 2544)
        || matches!(octets, [192, 0, 0, _]) // IETF Protocol Assignments (RFC 6890)
        || matches!(octets, [192, 0, 2, _]) // TEST-NET-1 (RFC 5737)
        || matches!(octets, [198, 51, 100, _]) // TEST-NET-2 (RFC 5737)
        || matches!(octets, [203, 0, 113, _]) // TEST-NET-3 (RFC 5737)
        || matches!(octets, [192, 88, 99, _]) // 6to4 Anycast Relay (RFC 7526)
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    // IPv4-mapped (::ffff:x.x.x.x) — extract and check embedded v4.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_private_v4(v4);
    }
    // IPv4-compatible (::x.x.x.x, deprecated RFC 4291) — first 96 bits zero.
    if ip.is_unspecified() {
        return true;
    }
    let segs = ip.segments();
    if segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 && segs[4] == 0 && segs[5] == 0
    {
        // Last 32 bits encode an IPv4 address (two 16-bit segments → four 8-bit octets).
        let v4 = Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            segs[6] as u8,
            (segs[7] >> 8) as u8,
            segs[7] as u8,
        );
        return is_private_v4(v4);
    }
    // 6to4 (2002::/16) — segs[1] and segs[2] contain embedded IPv4
    if segs[0] == 0x2002 {
        let v4 = Ipv4Addr::new(
            (segs[1] >> 8) as u8,
            segs[1] as u8,
            (segs[2] >> 8) as u8,
            segs[2] as u8,
        );
        if is_private_v4(v4) {
            return true;
        }
    }
    // Teredo (2001:0000::/32) — client IPv4 is XOR-inverted in segs[6..7]
    if segs[0] == 0x2001 && segs[1] == 0x0000 {
        let inv_6 = !segs[6];
        let inv_7 = !segs[7];
        let v4 = Ipv4Addr::new(
            (inv_6 >> 8) as u8,
            inv_6 as u8,
            (inv_7 >> 8) as u8,
            inv_7 as u8,
        );
        if is_private_v4(v4) {
            return true;
        }
    }
    // Documentation (2001:db8::/32) & Benchmarking (2001:2::/48) & Discard (100::/64)
    if segs[0] == 0x2001 && segs[1] == 0x0db8 {
        return true;
    }
    if segs[0] == 0x2001 && segs[1] == 0x0002 {
        return true;
    }
    if segs[0] == 0x0100 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 {
        return true;
    }

    ip.is_loopback()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || segs[0] & 0xfe00 == 0xfc00
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
    fn allows_public_https_ip_literal_and_pins_addr() {
        let pinned = validate_and_pin_public_http_url("https://1.1.1.1/").unwrap();
        assert_eq!(pinned.addrs.len(), 1);
        assert_eq!(pinned.addrs[0].port(), 443);
        assert!(validate_public_http_url("https://8.8.8.8/dns").is_ok());
    }

    #[test]
    fn rejects_nonstandard_public_port() {
        assert!(validate_public_http_url("https://1.1.1.1:8443/").is_err());
    }

    #[test]
    fn rejects_internal_and_metadata_hosts() {
        assert!(validate_public_http_url("http://metadata.google.internal/").is_err());
        assert!(validate_public_http_url("http://foo.internal/bar").is_err());
        assert!(validate_public_http_url("http://router.lan/config").is_err());
        assert!(validate_public_http_url("http://payroll.corp/auth").is_err());
        assert!(validate_public_http_url("http://device.home/admin").is_err());
        assert!(validate_public_http_url("http://gateway.intranet/api").is_err());
        assert!(validate_public_http_url("http://secret.onion/").is_err());
        assert!(validate_public_http_url("http://instance-data/latest/meta-data").is_err());
        assert!(validate_public_http_url("http://ip6-localhost/").is_err());
    }

    #[test]
    fn rejects_public_ipv4_literals_that_are_private() {
        assert!(validate_public_http_url("https://169.254.169.254/latest").is_err());
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6() {
        assert!(validate_public_http_url("http://[::ffff:127.0.0.1]/").is_err());
        assert!(validate_public_http_url("http://[::ffff:10.0.0.1]/").is_err());
        assert!(validate_public_http_url("http://[::ffff:192.168.1.1]/").is_err());
    }

    #[test]
    fn rejects_ipv4_compatible_ipv6() {
        assert!(validate_public_http_url("http://[::127.0.0.1]/").is_err());
        assert!(validate_public_http_url("http://[::10.0.0.1]/").is_err());
        assert!(validate_public_http_url("http://[::192.168.1.1]/").is_err());
        assert!(validate_public_http_url("http://[::169.254.169.254]/").is_err());
    }

    #[test]
    fn rejects_multicast_ipv4() {
        assert!(validate_public_http_url("http://224.0.0.1/").is_err());
        assert!(validate_public_http_url("http://239.255.255.250/").is_err());
    }

    #[test]
    fn rejects_multicast_ipv6() {
        assert!(validate_public_http_url("http://[ff02::1]/").is_err());
    }

    #[test]
    fn rejects_class_e_and_benchmark_ranges() {
        assert!(validate_public_http_url("http://240.0.0.1/").is_err());
        assert!(validate_public_http_url("http://250.1.2.3/").is_err());
        assert!(validate_public_http_url("http://198.18.0.1/").is_err());
        assert!(validate_public_http_url("http://198.19.255.255/").is_err());
        assert!(validate_public_http_url("http://192.0.0.1/").is_err());
    }

    #[test]
    fn rejects_ipv6_documentation_and_tunneling_private_ips() {
        // Documentation prefix (2001:db8::/32)
        assert!(validate_public_http_url("http://[2001:db8::1]/").is_err());
        // 6to4 (2002::/16) encapsulating 10.0.0.1 (0a00:0001)
        assert!(validate_public_http_url("http://[2002:0a00:0001::]/").is_err());
        // 6to4 encapsulating 192.168.1.1 (c0a8:0101)
        assert!(validate_public_http_url("http://[2002:c0a8:0101::]/").is_err());
    }
}
