"""LRU cleanup for managed crawler cache."""

from __future__ import annotations

import time
from typing import Any

from .cache import DEFAULT_CACHE_QUOTA_BYTES, cache_stats, list_cache_files
from .resources import data_root


def cleanup_cache(
    *,
    quota_bytes: int = DEFAULT_CACHE_QUOTA_BYTES,
    target_ratio: float = 0.8,
) -> dict[str, Any]:
    root = data_root()
    before = cache_stats(quota_bytes)
    removed = 0
    freed = 0
    target = int(quota_bytes * target_ratio)
    size = before["sizeBytes"]
    if size <= target:
        return {
            "removedFiles": 0,
            "freedBytes": 0,
            "before": before,
            "after": before,
            "lastCleanup": time.time(),
        }

    for path, file_size, _mtime in list_cache_files(root):
        if size <= target:
            break
        # Keep DB file; prefer content/html/screenshot/tmp.
        name = path.name.lower()
        if name.endswith(".db") or name.endswith(".db-wal") or name.endswith(".db-shm"):
            continue
        try:
            path.unlink(missing_ok=True)
            removed += 1
            freed += file_size
            size -= file_size
        except OSError:
            continue

    after = cache_stats(quota_bytes)
    return {
        "removedFiles": removed,
        "freedBytes": freed,
        "before": before,
        "after": after,
        "lastCleanup": time.time(),
    }
