#!/usr/bin/env python3
"""Coreside crawler doctor / stats wrapper."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
os.environ.setdefault("CRAWL4_AI_BASE_DIRECTORY", str(ROOT / ".crawler-data"))


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "doctor"
    if mode == "stats":
        from coreside_crawler.cache import cache_stats
        from coreside_crawler.resources import resource_stats
        from coreside_crawler.configs import resolve_resource_profile

        print(json.dumps({"cache": cache_stats(), "resource": resource_stats(resolve_resource_profile("balanced"), active_pages=0, queued=0)}, indent=2))
        return 0
    doctor = ROOT / (".venv/Scripts/crawl4ai-doctor.exe" if os.name == "nt" else ".venv/bin/crawl4ai-doctor")
    if not doctor.exists():
        print("crawl4ai-doctor missing; run npm run crawl4ai:setup", file=sys.stderr)
        return 1
    return subprocess.call([str(doctor)])


if __name__ == "__main__":
    raise SystemExit(main())
