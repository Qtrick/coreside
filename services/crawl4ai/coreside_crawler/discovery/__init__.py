"""Discovery adapters — Stage 1 URL finding (not a global web index)."""

from __future__ import annotations

from .direct_url import discover_direct_urls
from .page_links import discover_from_links
from .ranking import rank_candidates
from .rss import discover_rss
from .sitemap import discover_sitemap

__all__ = [
    "discover_direct_urls",
    "discover_from_links",
    "discover_rss",
    "discover_sitemap",
    "rank_candidates",
]
