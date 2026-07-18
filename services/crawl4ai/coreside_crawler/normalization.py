"""Normalize Crawl4AI results into Coreside source / media records."""

from __future__ import annotations

import hashlib
import re
from datetime import datetime, timezone
from typing import Any
from urllib.parse import urljoin, urlparse

from .configs import HARD_MAX_EXTRACTED_CHARS, HARD_MAX_MARKDOWN_CHARS, HARD_MAX_MEDIA_RESULTS


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _domain(url: str) -> str:
    try:
        return urlparse(url).hostname or ""
    except Exception:
        return ""


def _clip(text: str | None, limit: int) -> str:
    if not text:
        return ""
    t = text.strip()
    if len(t) <= limit:
        return t
    return t[: limit - 1].rstrip() + "…"


def content_hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8", errors="ignore")).hexdigest()


def normalize_source(result: Any, *, profile: str, cache_status: str = "live") -> dict[str, Any]:
    url = getattr(result, "url", "") or ""
    redirected = getattr(result, "redirected_url", None) or url
    markdown = ""
    md = getattr(result, "markdown", None)
    if md is not None:
        if hasattr(md, "fit_markdown") and md.fit_markdown:
            markdown = str(md.fit_markdown)
        elif hasattr(md, "raw_markdown") and md.raw_markdown:
            markdown = str(md.raw_markdown)
        else:
            markdown = str(md)
    markdown = _clip(markdown, HARD_MAX_MARKDOWN_CHARS)
    excerpt = _clip(markdown, min(4000, HARD_MAX_EXTRACTED_CHARS))

    metadata = getattr(result, "metadata", None) or {}
    if not isinstance(metadata, dict):
        metadata = {}

    title = metadata.get("title") or metadata.get("og:title") or _domain(url) or url
    description = metadata.get("description") or metadata.get("og:description") or excerpt[:280]
    status = getattr(result, "status_code", None)
    success = bool(getattr(result, "success", False))

    links = getattr(result, "links", None) or {}
    link_count = 0
    if isinstance(links, dict):
        link_count = len(links.get("internal") or []) + len(links.get("external") or [])

    media = getattr(result, "media", None) or {}
    media_counts = {"images": 0, "videos": 0, "audios": 0}
    if isinstance(media, dict):
        for key, out in (("images", "images"), ("videos", "videos"), ("audios", "audios")):
            items = media.get(key) or []
            media_counts[out] = len(items) if isinstance(items, list) else 0

    return {
        "sourceId": content_hash(redirected or url)[:24],
        "url": url,
        "finalUrl": redirected,
        "canonicalUrl": metadata.get("canonical") or redirected,
        "title": str(title)[:500],
        "description": str(description)[:1000],
        "domain": _domain(redirected or url),
        "author": metadata.get("author"),
        "publishedAt": metadata.get("published_time") or metadata.get("article:published_time"),
        "modifiedAt": metadata.get("modified_time"),
        "crawledAt": _now_iso(),
        "statusCode": status,
        "contentType": metadata.get("content_type"),
        "language": metadata.get("language"),
        "contentHash": content_hash(markdown),
        "excerpt": excerpt,
        "markdown": markdown,
        "truncated": True,
        "crawlMethod": profile,
        "cacheStatus": cache_status,
        "robotsStatus": "allowed" if success else "unknown",
        "failureCategory": None if success else "unknown",
        "mediaCounts": media_counts,
        "linkCounts": {"total": link_count},
        "success": success,
    }


def extract_links(result: Any, *, base_url: str, limit: int = 50) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    seen: set[str] = set()
    links = getattr(result, "links", None) or {}
    buckets = []
    if isinstance(links, dict):
        buckets.extend(links.get("internal") or [])
        buckets.extend(links.get("external") or [])
    for item in buckets:
        if len(out) >= limit:
            break
        if isinstance(item, dict):
            href = item.get("href") or item.get("url") or ""
            text = item.get("text") or item.get("title") or ""
        else:
            href = str(item)
            text = ""
        if not href:
            continue
        abs_url = urljoin(base_url, href)
        if abs_url in seen:
            continue
        seen.add(abs_url)
        out.append({"url": abs_url, "text": str(text)[:200]})
    return out


_CAPTCHA_MARKERS = re.compile(
    r"(captcha|cf-challenge|challenge-platform|verify you are human|attention required)",
    re.I,
)
_LOGIN_MARKERS = re.compile(r"(sign in to continue|log in to continue|auth0|oauth)", re.I)


def detect_anti_bot(markdown: str, html_hint: str = "") -> str | None:
    blob = f"{markdown}\n{html_hint}"
    if _CAPTCHA_MARKERS.search(blob):
        return "captcha_required" if "captcha" in blob.lower() else "anti_bot_blocked"
    if _LOGIN_MARKERS.search(blob) and len(markdown.strip()) < 400:
        return "login_required"
    return None
