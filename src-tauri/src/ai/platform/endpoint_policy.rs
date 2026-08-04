//! Endpoint security policy for provider connections.

use super::types::EndpointClass;
use std::net::IpAddr;
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
}

/// Validate a candidate base URL for the given endpoint class.
///
/// Never sends credentials through redirects — that is enforced by HTTP clients.
/// This function only classifies and accepts/rejects the configured destination.
pub fn classify_and_validate_endpoint(
    raw_url: &str,
    endpoint_class: EndpointClass,
    allows_override: bool,
    is_override: bool,
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
    let host = url
        .host_str()
        .ok_or(EndpointPolicyError::InvalidUrl)?
        .to_lowercase();
    let scheme = url.scheme().to_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or(EndpointPolicyError::PortNotAllowed)?;

    let ip = host.parse::<IpAddr>().ok();
    let is_loopback = host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || ip.map(|a| a.is_loopback()).unwrap_or(false);
    let is_private = ip.map(is_private_or_shared).unwrap_or(false);
    let is_link_local_or_metadata = ip.map(is_blocked_special).unwrap_or(false)
        || host == "metadata.google.internal"
        || host.ends_with(".metadata.google.internal");

    if is_link_local_or_metadata {
        return Err(EndpointPolicyError::MetadataOrLinkLocalBlocked);
    }

    match endpoint_class {
        EndpointClass::FixedTrustedRemote
        | EndpointClass::VettedRemoteProviderPreset
        | EndpointClass::UserConfiguredRemoteCompatible
        | EndpointClass::HostedCoresideGateway => {
            if scheme != "https" {
                return Err(EndpointPolicyError::RemoteHttpForbidden);
            }
            if is_loopback || is_private {
                return Err(EndpointPolicyError::HostNotAllowed);
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
                // Loopback is fine under LAN mode too, but prefer LoopbackLocal.
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
    })
}

fn is_private_or_shared(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_link_local()
                // 100.64.0.0/10 shared address space
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                // 198.18.0.0/15 benchmarking
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        }
        IpAddr::V6(v6) => {
            // Unique local (fc00::/7) and link-local.
            (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn is_blocked_special(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // 169.254.0.0/16 link-local + 169.254.169.254 cloud metadata.
            v4.is_link_local() || v4.octets() == [169, 254, 169, 254]
        }
        IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
