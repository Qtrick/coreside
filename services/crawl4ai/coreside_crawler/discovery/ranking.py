"""Deterministic candidate ranking — not LLM ranking."""

from __future__ import annotations

from typing import Any
from urllib.parse import urlparse


def rank_candidates(
    candidates: list[dict[str, Any]],
    *,
    query: str = "",
    limit: int = 20,
) -> list[dict[str, Any]]:
    q_terms = [t for t in (query or "").lower().split() if t]
    scored: list[dict[str, Any]] = []
    seen: set[str] = set()
    for c in candidates:
        url = (c.get("url") or "").strip()
        if not url or url in seen:
            continue
        seen.add(url)
        title = (c.get("title") or "").lower()
        snippet = (c.get("snippet") or "").lower()
        domain = urlparse(url).hostname or ""
        overlap = 0
        for t in q_terms:
            if t in title:
                overlap += 3
            if t in snippet:
                overlap += 2
            if t in url.lower() or t in domain:
                overlap += 1
        score = float(c.get("score") or 0) + overlap
        # Prefer official-looking docs/repos lightly.
        if any(x in domain for x in ("github.com", "docs.", "developer.", "wikipedia.org")):
            score += 0.5
        item = dict(c)
        item["score"] = score
        scored.append(item)
    scored.sort(key=lambda x: x["score"], reverse=True)
    return scored[:limit]
