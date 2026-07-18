"""Expand candidates from already-known page links."""

from __future__ import annotations

from typing import Any


def discover_from_links(links: list[dict[str, str]], *, query: str = "", limit: int = 20) -> list[dict[str, Any]]:
    q = (query or "").lower().split()
    scored: list[dict[str, Any]] = []
    for link in links:
        url = link.get("url") or ""
        text = link.get("text") or ""
        if not url:
            continue
        blob = f"{url} {text}".lower()
        overlap = sum(1 for t in q if t and t in blob)
        scored.append(
            {
                "url": url,
                "title": text[:200],
                "snippet": text[:400],
                "source": "page_links",
                "score": float(overlap) + 0.1,
            }
        )
    scored.sort(key=lambda x: x["score"], reverse=True)
    return scored[:limit]
