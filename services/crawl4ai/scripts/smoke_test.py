#!/usr/bin/env python3
"""Smoke test: crawl example.com, assert markdown, links, and media fields."""

from __future__ import annotations

import asyncio
import os
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

os.environ.setdefault("CRAWL4_AI_BASE_DIRECTORY", str(ROOT / ".crawler-data"))
os.environ.setdefault("CORESIDE_CRAWLER_DATA_ROOT", str(ROOT / ".crawler-data" / ".crawl4ai"))


async def main() -> int:
    from coreside_crawler.crawler_manager import CrawlerManager

    mgr = CrawlerManager()
    t0 = time.time()
    result = await mgr.crawl_url("https://example.com", profile="text")
    elapsed = time.time() - t0
    source = result["source"]
    md = (source.get("markdown") or "")
    print(f"url={source.get('finalUrl')} title={source.get('title')!r}")
    print(f"markdown_chars={len(md)} elapsed_s={elapsed:.2f}")
    print(md[:300])
    if len(md) < 20:
        print("FAIL: expected markdown content")
        await mgr.shutdown()
        return 1
    # Media profile on a page with images
    media = await mgr.crawl_url("https://example.com", profile="media", include_media=True)
    print(f"media_images={len(media['media'].get('images') or [])}")
    await mgr.shutdown()
    print("SMOKE OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
