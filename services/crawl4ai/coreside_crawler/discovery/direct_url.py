"""Direct URL discovery."""

from __future__ import annotations

from typing import Any


def discover_direct_urls(urls: list[str]) -> list[dict[str, Any]]:
    out = []
    for u in urls:
        u = (u or "").strip()
        if not u:
            continue
        out.append(
            {
                "url": u,
                "title": "",
                "snippet": "",
                "source": "direct_url",
                "score": 1.0,
            }
        )
    return out
