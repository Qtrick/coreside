"""URL security — reject private/local targets. Production never allows loopback."""

from __future__ import annotations

import ipaddress
import os
import socket
from urllib.parse import urlparse

from .errors import CrawlError, SSRF_BLOCKED

BLOCKED_SCHEMES = frozenset(
    {"file", "ftp", "data", "javascript", "chrome", "about", "blob", "ws", "wss"}
)
BLOCKED_HOSTS = frozenset(
    {
        "localhost",
        "localhost.localdomain",
        "metadata.google.internal",
        "metadata.goog",
    }
)

# Carrier-grade NAT / shared address space (not covered by ipaddress.is_private).
_CGNAT_V4 = ipaddress.ip_network("100.64.0.0/10")


def _loopback_tests_enabled(flag: bool) -> bool:
    """Loopback is allowed only when both the call flag and env gate are set."""
    if not flag:
        return False
    return os.environ.get("CORESIDE_CRAWLER_ALLOW_LOOPBACK", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }


def validate_public_http_url(raw: str, *, allow_loopback_for_tests: bool = False) -> str:
    trimmed = (raw or "").strip()
    if not trimmed:
        raise CrawlError(SSRF_BLOCKED, "URL is required")

    allow_loopback = _loopback_tests_enabled(allow_loopback_for_tests)

    parsed = urlparse(trimmed)
    scheme = (parsed.scheme or "").lower()
    if scheme in BLOCKED_SCHEMES or scheme not in {"http", "https"}:
        raise CrawlError(SSRF_BLOCKED, f"unsupported protocol: {scheme or 'none'}")

    if parsed.username or parsed.password:
        raise CrawlError(SSRF_BLOCKED, "embedded credentials are not allowed")

    host = (parsed.hostname or "").lower()
    if not host:
        raise CrawlError(SSRF_BLOCKED, "missing host")

    if host in BLOCKED_HOSTS or host.endswith((".localhost", ".local", ".internal")):
        if not (allow_loopback and host in {"localhost", "127.0.0.1"}):
            raise CrawlError(SSRF_BLOCKED, f"blocked host: {host}")

    try:
        ip = ipaddress.ip_address(host)
        if _is_private_or_reserved(ip) and not (allow_loopback and ip.is_loopback):
            raise CrawlError(SSRF_BLOCKED, f"private IP: {ip}")
        return trimmed
    except ValueError:
        pass

    if not allow_loopback:
        _resolve_public(host)

    return trimmed


def _resolve_public(host: str) -> None:
    try:
        infos = socket.getaddrinfo(host, None)
    except socket.gaierror as e:
        raise CrawlError(SSRF_BLOCKED, f"DNS resolution failed for {host}: {e}") from e
    if not infos:
        raise CrawlError(SSRF_BLOCKED, f"DNS returned no addresses for {host}")
    for info in infos:
        ip = ipaddress.ip_address(info[4][0])
        if _is_private_or_reserved(ip):
            raise CrawlError(SSRF_BLOCKED, f"host {host} resolves to private IP: {ip}")


def _is_private_or_reserved(ip: ipaddress._BaseAddress) -> bool:
    if isinstance(ip, ipaddress.IPv4Address) and ip in _CGNAT_V4:
        return True
    if isinstance(ip, ipaddress.IPv4Address) and ip == ipaddress.IPv4Address("255.255.255.255"):
        return True
    return bool(
        ip.is_private
        or ip.is_loopback
        or ip.is_link_local
        or ip.is_reserved
        or ip.is_multicast
        or ip.is_unspecified
    )
