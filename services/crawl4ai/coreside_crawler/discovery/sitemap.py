"""Sitemap discovery — fetch and parse public sitemaps (bounded)."""

from __future__ import annotations

import re
import xml.etree.ElementTree as ET
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urljoin, urlparse
from urllib.request import Request, build_opener, HTTPRedirectHandler

from .. import CORESIDE_USER_AGENT
from ..errors import CrawlError
from ..security import validate_public_http_url

_LOC_RE = re.compile(r"<loc>\s*([^<]+)\s*</loc>", re.I)


class _ValidatedRedirectHandler(HTTPRedirectHandler):
    """Re-validate every redirect hop so sitemap fetches cannot SSRF via Location."""

    def __init__(self, *, allow_loopback_for_tests: bool = False) -> None:
        super().__init__()
        self._allow_loopback = allow_loopback_for_tests

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # type: ignore[no-untyped-def]
        validate_public_http_url(newurl, allow_loopback_for_tests=self._allow_loopback)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def _fetch_text(url: str, *, allow_loopback_for_tests: bool) -> str:
    validate_public_http_url(url, allow_loopback_for_tests=allow_loopback_for_tests)
    opener = build_opener(
        _ValidatedRedirectHandler(allow_loopback_for_tests=allow_loopback_for_tests)
    )
    req = Request(url, headers={"User-Agent": CORESIDE_USER_AGENT})
    with opener.open(req, timeout=10) as resp:  # noqa: S310 — validated URL + redirects
        final = getattr(resp, "geturl", lambda: url)()
        validate_public_http_url(final, allow_loopback_for_tests=allow_loopback_for_tests)
        return resp.read(500_000).decode("utf-8", errors="ignore")


def discover_sitemap(seed_url: str, *, limit: int = 20, allow_loopback_for_tests: bool = False) -> list[dict[str, Any]]:
    validate_public_http_url(seed_url, allow_loopback_for_tests=allow_loopback_for_tests)
    parsed = urlparse(seed_url)
    base = f"{parsed.scheme}://{parsed.netloc}"
    candidates = [
        urljoin(base, "/sitemap.xml"),
        urljoin(base, "/sitemap_index.xml"),
        urljoin(base, "/robots.txt"),
    ]
    urls: list[str] = []
    for cand in candidates:
        try:
            body = _fetch_text(cand, allow_loopback_for_tests=allow_loopback_for_tests)
        except (CrawlError, HTTPError, URLError, TimeoutError, OSError, ValueError):
            continue
        if cand.endswith("robots.txt"):
            for line in body.splitlines():
                if line.lower().startswith("sitemap:"):
                    sm = line.split(":", 1)[1].strip()
                    urls.extend(
                        _fetch_sitemap_locs(
                            sm, limit=limit, allow_loopback_for_tests=allow_loopback_for_tests
                        )
                    )
        else:
            urls.extend(_parse_locs(body))
        if len(urls) >= limit:
            break
    out = []
    seen = set()
    for u in urls:
        if u in seen:
            continue
        try:
            validate_public_http_url(u, allow_loopback_for_tests=allow_loopback_for_tests)
        except CrawlError:
            continue
        seen.add(u)
        out.append({"url": u, "title": "", "snippet": "", "source": "sitemap", "score": 0.5})
        if len(out) >= limit:
            break
    return out


def _fetch_sitemap_locs(url: str, *, limit: int, allow_loopback_for_tests: bool) -> list[str]:
    try:
        body = _fetch_text(url, allow_loopback_for_tests=allow_loopback_for_tests)
        return _parse_locs(body)[:limit]
    except (CrawlError, HTTPError, URLError, TimeoutError, OSError, ValueError):
        return []


def _parse_locs(body: str) -> list[str]:
    locs = _LOC_RE.findall(body)
    if locs:
        return [x.strip() for x in locs if x.strip().startswith("http")]
    try:
        root = ET.fromstring(body)
        return [el.text.strip() for el in root.iter() if el.tag.endswith("loc") and el.text]
    except ET.ParseError:
        return []
