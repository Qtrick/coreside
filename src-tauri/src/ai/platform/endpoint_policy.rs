//! Endpoint security policy for provider connections.
//!
//! Remote credential-bearing destinations resolve DNS and reject any private
//! A/AAAA answer (OWASP SSRF). Local AI classes allow loopback / private LAN
//! only under the explicit endpoint class. Redirect policy is enforced by
//! HTTP clients (`Policy::none()`); credentials must not be attached until
//! after this validation succeeds.

use super::types::EndpointClass;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EndpointPolicyError {
    #[error("endpoint URL is required")]
    MissingUrl,
    #[error("invalid endpoint URL")]
    InvalidUrl,
    #[error("URL userinfo is not allowed")]
    UserInfoForbidden,
    #[error("URL fragments are not allowed")]
    FragmentForbidden,
    #[error("remote endpoints require HTTPS")]
    RemoteHttpForbidden,
    #[error("loopback Local AI may use HTTP only for loopback hosts")]
    NonLoopbackHttpForbidden,
    #[error("host is not allowed for this endpoint class")]
    HostNotAllowed,
    #[error("private or link-local destinations require explicit private-LAN Local AI mode")]
    PrivateNetworkRequiresLanMode,
    #[error("metadata or link-local addresses are blocked")]
    MetadataOrLinkLocalBlocked,
    #[error("port is not allowed")]
    PortNotAllowed,
    #[error("this provider does not allow custom endpoint overrides")]
    OverrideNotAllowed,
    #[error("DNS resolution failed or returned no public addresses")]
    DnsResolutionFailed,
    #[error("host resolves to a blocked address")]
    ResolvedAddressBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointValidation {
    pub origin: String,
    pub host: String,
    pub port: u16,
    pub scheme: String,
    pub endpoint_class: EndpointClass,
    pub is_loopback: bool,
    pub is_private: bool,
    /// Resolved socket addresses pinned at validation time (empty for unresolved local literals already classified).
    pub pinned_addrs: Vec<SocketAddr>,
}

type ResolveFn = dyn Fn(&str, u16) -> Result<Vec<SocketAddr>, EndpointPolicyError>;

/// Validate a candidate base URL for the given endpoint class.
pub fn classify_and_validate_endpoint(
    raw_url: &str,
    endpoint_class: EndpointClass,
    allows_override: bool,
    is_override: bool,
) -> Result<EndpointValidation, EndpointPolicyError> {
    classify_and_validate_endpoint_with_resolver(
        raw_url,
        endpoint_class,
        allows_override,
        is_override,
        &default_resolve,
    )
}

/// Injectable resolver for adversarial DNS / rebinding tests.
pub fn classify_and_validate_endpoint_with_resolver(
    raw_url: &str,
    endpoint_class: EndpointClass,
    allows_override: bool,
    is_override: bool,
    resolve: &ResolveFn,
) -> Result<EndpointValidation, EndpointPolicyError> {
    if is_override && !allows_override {
        return Err(EndpointPolicyError::OverrideNotAllowed);
    }
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(EndpointPolicyError::MissingUrl);
    }
    let url = Url::parse(trimmed).map_err(|_| EndpointPolicyError::InvalidUrl)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(EndpointPolicyError::UserInfoForbidden);
    }
    if url.fragment().is_some() {
        return Err(EndpointPolicyError::FragmentForbidden);
    }
    let host = url
        .host_str()
        .ok_or(EndpointPolicyError::InvalidUrl)?
        .to_lowercase();
    let scheme = url.scheme().to_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or(EndpointPolicyError::PortNotAllowed)?;

    let literal_ip = parse_host_ip(&host);
    let is_loopback_literal = host == "localhost"
        || host == "localhost.localdomain"
        || literal_ip.map(|a| a.is_loopback()).unwrap_or(false);
    let is_private_literal = literal_ip.map(is_blocked_destination_ip).unwrap_or(false)
        && !literal_ip.map(|a| a.is_loopback()).unwrap_or(false);
    let is_metadata_literal = literal_ip.map(is_metadata_or_link_local).unwrap_or(false)
        || host == "metadata.google.internal"
        || host.ends_with(".metadata.google.internal")
        || host == "metadata.goog";

    if is_metadata_literal {
        return Err(EndpointPolicyError::MetadataOrLinkLocalBlocked);
    }

    let requires_public_resolve = matches!(
        endpoint_class,
        EndpointClass::FixedTrustedRemote
            | EndpointClass::VettedRemoteProviderPreset
            | EndpointClass::UserConfiguredRemoteCompatible
            | EndpointClass::HostedCoresideGateway
    );

    match endpoint_class {
        EndpointClass::FixedTrustedRemote
        | EndpointClass::VettedRemoteProviderPreset
        | EndpointClass::UserConfiguredRemoteCompatible
        | EndpointClass::HostedCoresideGateway => {
            // Fail closed on scheme before DNS — clearer errors, no network on http://.
            if scheme != "https" {
                return Err(EndpointPolicyError::RemoteHttpForbidden);
            }
        }
        _ => {}
    }

    let mut pinned_addrs: Vec<SocketAddr> = Vec::new();
    let mut is_loopback = is_loopback_literal;
    let mut is_private = is_private_literal;

    if let Some(ip) = literal_ip {
        pinned_addrs.push(SocketAddr::new(ip, port));
        is_loopback = ip.is_loopback();
        is_private = is_blocked_destination_ip(ip) && !ip.is_loopback();
    } else if requires_public_resolve {
        // Hostname: resolve every A/AAAA and reject if any answer is prohibited.
        let addrs = resolve(&host, port)?;
        if addrs.is_empty() {
            return Err(EndpointPolicyError::DnsResolutionFailed);
        }
        for addr in &addrs {
            let ip = addr.ip();
            if is_metadata_or_link_local(ip) {
                return Err(EndpointPolicyError::MetadataOrLinkLocalBlocked);
            }
            if is_blocked_destination_ip(ip) {
                return Err(EndpointPolicyError::ResolvedAddressBlocked);
            }
        }
        pinned_addrs = addrs;
        is_loopback = false;
        is_private = false;
    } else {
        // Local AI hostname (e.g. custom.local): resolve and classify.
        match resolve(&host, port) {
            Ok(addrs) if !addrs.is_empty() => {
                let mut any_loopback = false;
                let mut any_private = false;
                let mut any_public = false;
                for addr in &addrs {
                    let ip = addr.ip();
                    if is_metadata_or_link_local(ip) {
                        return Err(EndpointPolicyError::MetadataOrLinkLocalBlocked);
                    }
                    if ip.is_loopback() {
                        any_loopback = true;
                    } else if is_blocked_destination_ip(ip) {
                        any_private = true;
                    } else {
                        any_public = true;
                    }
                }
                // Mixed public+private answers are unsafe for Local AI credential paths.
                if any_public && (any_loopback || any_private) {
                    return Err(EndpointPolicyError::ResolvedAddressBlocked);
                }
                if any_public {
                    is_loopback = false;
                    is_private = false;
                } else {
                    is_loopback = any_loopback && !any_private;
                    is_private = any_private;
                }
                pinned_addrs = addrs;
            }
            _ => {
                // Unresolvable local hostnames: fall back to hostname heuristics.
                if host.ends_with(".local") || host.ends_with(".internal") || host == "localhost" {
                    is_loopback = host == "localhost" || host.ends_with(".localhost");
                    is_private = !is_loopback;
                }
            }
        }
    }

    match endpoint_class {
        EndpointClass::FixedTrustedRemote
        | EndpointClass::VettedRemoteProviderPreset
        | EndpointClass::UserConfiguredRemoteCompatible
        | EndpointClass::HostedCoresideGateway => {
            if is_loopback || is_private {
                return Err(EndpointPolicyError::HostNotAllowed);
            }
            if pinned_addrs.is_empty() && literal_ip.is_none() {
                return Err(EndpointPolicyError::DnsResolutionFailed);
            }
        }
        EndpointClass::LoopbackLocal => {
            if !is_loopback {
                if is_private {
                    return Err(EndpointPolicyError::PrivateNetworkRequiresLanMode);
                }
                return Err(EndpointPolicyError::HostNotAllowed);
            }
            if scheme != "http" && scheme != "https" {
                return Err(EndpointPolicyError::InvalidUrl);
            }
            if scheme == "http" && !is_loopback {
                return Err(EndpointPolicyError::NonLoopbackHttpForbidden);
            }
        }
        EndpointClass::PrivateLanLocal => {
            if is_loopback {
                // Loopback is fine under LAN mode too.
            } else if !is_private {
                return Err(EndpointPolicyError::HostNotAllowed);
            }
            if scheme != "http" && scheme != "https" {
                return Err(EndpointPolicyError::InvalidUrl);
            }
        }
    }

    let origin = format!("{scheme}://{host}:{port}");
    Ok(EndpointValidation {
        origin,
        host,
        port,
        scheme,
        endpoint_class,
        is_loopback,
        is_private,
        pinned_addrs,
    })
}

fn default_resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, EndpointPolicyError> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|_| EndpointPolicyError::DnsResolutionFailed)?;
    let out: Vec<SocketAddr> = addrs.map(|a| SocketAddr::new(a.ip(), port)).collect();
    if out.is_empty() {
        return Err(EndpointPolicyError::DnsResolutionFailed);
    }
    Ok(out)
}

fn parse_host_ip(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    if host.starts_with('[') && host.ends_with(']') {
        return host[1..host.len() - 1].parse::<IpAddr>().ok();
    }
    None
}

/// Shared public-destination classifier aligned with search SSRF policy.
pub fn is_blocked_destination_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || o[0] == 0
        || matches!(o, [100, 64..=127, _, _]) // shared CGNAT
        || matches!(o, [198, 18..=19, _, _]) // benchmarking
        || matches!(o, [192, 0, 2, _]) // TEST-NET-1
        || matches!(o, [198, 51, 100, _]) // TEST-NET-2
        || matches!(o, [203, 0, 113, _]) // TEST-NET-3
}

fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_metadata_or_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local() || v4.octets() == [169, 254, 169, 254],
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_metadata_or_link_local(IpAddr::V4(v4));
            }
            v6.is_unicast_link_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn rejects_remote_http_with_credentials_destination() {
        let err = classify_and_validate_endpoint(
            "http://api.example.com/v1",
            EndpointClass::UserConfiguredRemoteCompatible,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::RemoteHttpForbidden);
    }

    #[test]
    fn accepts_loopback_http_for_local_ai() {
        let v = classify_and_validate_endpoint(
            "http://127.0.0.1:11434",
            EndpointClass::LoopbackLocal,
            true,
            true,
        )
        .unwrap();
        assert!(v.is_loopback);
        assert_eq!(v.port, 11434);
        assert!(!v.pinned_addrs.is_empty());
    }

    #[test]
    fn rejects_userinfo() {
        let err = classify_and_validate_endpoint(
            "https://user:pass@api.openai.com/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::UserInfoForbidden);
    }

    #[test]
    fn rejects_fragment() {
        let err = classify_and_validate_endpoint(
            "https://api.openai.com/v1#x",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::FragmentForbidden);
    }

    #[test]
    fn private_lan_required_for_rfc1918() {
        let err = classify_and_validate_endpoint(
            "http://192.168.1.10:11434",
            EndpointClass::LoopbackLocal,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::PrivateNetworkRequiresLanMode);
    }

    #[test]
    fn blocks_metadata_host() {
        let err = classify_and_validate_endpoint(
            "http://169.254.169.254/",
            EndpointClass::LoopbackLocal,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::MetadataOrLinkLocalBlocked);
    }

    #[test]
    fn rejects_loopback_http_for_remote_credential_class() {
        let err = classify_and_validate_endpoint(
            "http://127.0.0.1:11434",
            EndpointClass::UserConfiguredRemoteCompatible,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::RemoteHttpForbidden);
    }

    #[test]
    fn rejects_loopback_https_for_remote_class() {
        let err = classify_and_validate_endpoint(
            "https://127.0.0.1:443/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::HostNotAllowed);
    }

    #[test]
    fn rejects_public_http_in_loopback_local_mode() {
        let err = classify_and_validate_endpoint(
            "http://api.example.com/v1",
            EndpointClass::LoopbackLocal,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::HostNotAllowed);
    }

    #[test]
    fn accepts_private_lan_in_lan_mode() {
        let v = classify_and_validate_endpoint(
            "http://192.168.1.10:11434",
            EndpointClass::PrivateLanLocal,
            true,
            true,
        )
        .unwrap();
        assert!(v.is_private);
        assert!(!v.is_loopback);
        assert_eq!(v.port, 11434);
    }

    #[test]
    fn hostname_resolving_only_private_is_blocked_for_remote() {
        let resolve = |_host: &str, port: u16| {
            Ok(vec![SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(10, 0, 0, 1),
                port,
            ))])
        };
        let err = classify_and_validate_endpoint_with_resolver(
            "https://evil.example/v1",
            EndpointClass::UserConfiguredRemoteCompatible,
            true,
            true,
            &resolve,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::ResolvedAddressBlocked);
    }

    #[test]
    fn hostname_mixed_public_and_private_answers_blocked() {
        let resolve = |_host: &str, port: u16| {
            Ok(vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), port)),
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), port)),
            ])
        };
        let err = classify_and_validate_endpoint_with_resolver(
            "https://mixed.example/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
            &resolve,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::ResolvedAddressBlocked);
    }

    #[test]
    fn hostname_public_only_pins_addrs() {
        let resolve = |_host: &str, port: u16| {
            Ok(vec![SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(1, 1, 1, 1),
                port,
            ))])
        };
        let v = classify_and_validate_endpoint_with_resolver(
            "https://api.example.com/v1",
            EndpointClass::UserConfiguredRemoteCompatible,
            true,
            true,
            &resolve,
        )
        .unwrap();
        assert_eq!(v.pinned_addrs.len(), 1);
        assert_eq!(v.pinned_addrs[0].ip(), IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn ipv4_mapped_private_ipv6_blocked() {
        // ::ffff:192.168.1.1
        let ip = IpAddr::V6("::ffff:192.168.1.1".parse::<Ipv6Addr>().unwrap());
        assert!(is_blocked_destination_ip(ip));
        let err = classify_and_validate_endpoint(
            "https://[::ffff:192.168.1.1]/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            EndpointPolicyError::HostNotAllowed | EndpointPolicyError::ResolvedAddressBlocked
        ));
    }

    #[test]
    fn ipv6_unique_local_blocked_for_remote() {
        let err = classify_and_validate_endpoint(
            "https://[fd12:3456:789a::1]/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::HostNotAllowed);
    }

    #[test]
    fn ipv6_loopback_blocked_for_remote() {
        let err = classify_and_validate_endpoint(
            "https://[::1]/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::HostNotAllowed);
    }

    #[test]
    fn documentation_range_blocked() {
        assert!(is_blocked_destination_ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))));
        assert!(is_blocked_destination_ip(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))));
        assert!(is_blocked_destination_ip(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))));
    }

    #[test]
    fn link_local_ipv6_blocked() {
        let addr = SocketAddr::V6(SocketAddrV6::new(
            "fe80::1".parse().unwrap(),
            443,
            0,
            0,
        ));
        let resolve = move |_h: &str, _p: u16| Ok(vec![addr]);
        let err = classify_and_validate_endpoint_with_resolver(
            "https://ll.example/v1",
            EndpointClass::FixedTrustedRemote,
            false,
            false,
            &resolve,
        )
        .unwrap_err();
        assert_eq!(err, EndpointPolicyError::MetadataOrLinkLocalBlocked);
    }
}
