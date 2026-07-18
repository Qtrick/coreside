"""Domain seeding via homepage + sitemap + internal links (bounded)."""

from __future__ import annotations

from typing import Any
from urllib.parse import urlparse

from .page_links import discover_from_links
from .ranking import rank_candidates
from .sitemap import discover_sitemap


async def discover_domain(
    domain_or_url: str,
    *,
    query: str = "",
    limit: int = 20,
    crawl_fn=None,
    allow_loopback_for_tests: bool = False,
) -> list[dict[str, Any]]:
    raw = domain_or_url.strip()
    if not raw.startswith("http"):
        raw = f"https://{raw}"
    parsed = urlparse(raw)
    seed = f"{parsed.scheme}://{parsed.netloc}/"
    candidates: list[dict[str, Any]] = [{"url": seed, "title": parsed.netloc, "snippet": "", "source": "domain_seed", "score": 0.8}]
    try:
        candidates.extend(discover_sitemap(seed, limit=limit, allow_loopback_for_tests=allow_loopback_for_tests))
    except Exception:
        pass
    if crawl_fn:
        try:
            page = await crawl_fn(seed)
            links = page.get("links") or []
            candidates.extend(discover_from_links(links, query=query, limit=limit))
        except Exception:
            pass
    return rank_candidates(candidates, query=query, limit=limit)
