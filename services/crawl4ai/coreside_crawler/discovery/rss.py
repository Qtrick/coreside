"""RSS / Atom feed discovery."""

from __future__ import annotations

import re
from typing import Any
from urllib.error import HTTPError, URLError

from ..errors import CrawlError
from ..security import validate_public_http_url
from .sitemap import _fetch_text

_LINK_RE = re.compile(r"<link[^>]*href=[\"']([^\"']+)[\"']", re.I)
_ITEM_LINK_RE = re.compile(r"<item>.*?<link>([^<]+)</link>", re.I | re.S)
_ENTRY_RE = re.compile(r"<entry>.*?<link[^>]*href=[\"']([^\"']+)[\"']", re.I | re.S)


def discover_rss(feed_url: str, *, limit: int = 20, allow_loopback_for_tests: bool = False) -> list[dict[str, Any]]:
    try:
        body = _fetch_text(feed_url, allow_loopback_for_tests=allow_loopback_for_tests)
    except (CrawlError, HTTPError, URLError, TimeoutError, OSError, ValueError):
        return []
    urls = _ITEM_LINK_RE.findall(body) or _ENTRY_RE.findall(body) or _LINK_RE.findall(body)
    out = []
    seen = set()
    for u in urls:
        u = u.strip()
        if not u.startswith("http") or u in seen:
            continue
        try:
            validate_public_http_url(u, allow_loopback_for_tests=allow_loopback_for_tests)
        except CrawlError:
            continue
        seen.add(u)
        out.append({"url": u, "title": "", "snippet": "", "source": "rss", "score": 0.6})
        if len(out) >= limit:
            break
    return out
