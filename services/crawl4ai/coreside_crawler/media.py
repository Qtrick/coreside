"""Media extraction normalization — metadata only, no auto-download."""

from __future__ import annotations

from typing import Any
from urllib.parse import urljoin, urlparse

from .configs import HARD_MAX_MEDIA_RESULTS

MIN_IMAGE_DIMENSION = 64
MIN_WALLPAPER_DIMENSION = 800


def _abs(base: str, src: str | None) -> str | None:
    if not src:
        return None
    src = src.strip()
    if not src or src.startswith("data:"):
        return None
    parsed = urlparse(src)
    if parsed.scheme and parsed.scheme not in {"http", "https"}:
        return None
    return urljoin(base, src)


def normalize_media(result: Any, *, include_audio: bool = False) -> dict[str, list[dict[str, Any]]]:
    base = getattr(result, "url", "") or ""
    media = getattr(result, "media", None) or {}
    if not isinstance(media, dict):
        media = {}

    images = _normalize_images(media.get("images") or [], base)
    videos = _normalize_videos(media.get("videos") or [], base)
    audios: list[dict[str, Any]] = []
    if include_audio:
        audios = _normalize_audios(media.get("audios") or [], base)

    return {
        "images": images[:HARD_MAX_MEDIA_RESULTS],
        "videos": videos[:HARD_MAX_MEDIA_RESULTS],
        "audios": audios[:HARD_MAX_MEDIA_RESULTS],
    }


def _normalize_images(items: list, base: str) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict):
            continue
        url = _abs(base, item.get("src") or item.get("url"))
        if not url or url in seen:
            continue
        w = _as_int(item.get("width") or item.get("w"))
        h = _as_int(item.get("height") or item.get("h"))
        if w is not None and h is not None and (w < MIN_IMAGE_DIMENSION or h < MIN_IMAGE_DIMENSION):
            continue
        seen.add(url)
        out.append(
            {
                "url": url,
                "alt": (item.get("alt") or "")[:300],
                "description": (item.get("desc") or item.get("description") or "")[:500],
                "width": w,
                "height": h,
                "score": item.get("score"),
                "sourcePageUrl": base,
                "wallpaperCandidate": bool(
                    w and h and w >= MIN_WALLPAPER_DIMENSION and h >= MIN_WALLPAPER_DIMENSION
                ),
            }
        )
    return out


def _normalize_videos(items: list, base: str) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict):
            continue
        url = _abs(base, item.get("src") or item.get("url"))
        if not url or url in seen:
            continue
        seen.add(url)
        lower = url.lower()
        kind = "reference"
        if lower.endswith((".mp4", ".webm", ".mov", ".m4v")):
            kind = "direct"
        elif "m3u8" in lower or "mpd" in lower:
            kind = "stream_manifest"
        elif any(x in lower for x in ("youtube.com", "youtu.be", "vimeo.com")):
            kind = "watch_page"
        out.append(
            {
                "url": url,
                "title": (item.get("title") or item.get("alt") or "")[:300],
                "kind": kind,
                "sourcePageUrl": base,
                "importable": kind == "direct",
            }
        )
    return out


def _normalize_audios(items: list, base: str) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict):
            continue
        url = _abs(base, item.get("src") or item.get("url"))
        if not url or url in seen:
            continue
        seen.add(url)
        out.append({"url": url, "sourcePageUrl": base})
    return out


def _as_int(v: Any) -> int | None:
    try:
        if v is None or v == "":
            return None
        return int(float(v))
    except (TypeError, ValueError):
        return None
