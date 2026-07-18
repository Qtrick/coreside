"""Cache stats and modes (Coreside layer on top of Crawl4AI CacheMode)."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .resources import cache_size_bytes, data_root, disk_usage

DEFAULT_CACHE_QUOTA_BYTES = 1_000_000_000  # ~1 GB


def cache_stats(quota_bytes: int = DEFAULT_CACHE_QUOTA_BYTES) -> dict[str, Any]:
    root = data_root()
    size = cache_size_bytes(root)
    return {
        "root": str(root),
        "sizeBytes": size,
        "quotaBytes": quota_bytes,
        "usageRatio": (size / quota_bytes) if quota_bytes else 0,
        "disk": disk_usage(root),
        "overQuota": size > quota_bytes,
    }


def list_cache_files(root: Path | None = None) -> list[tuple[Path, int, float]]:
    """Return (path, size, mtime) for disposable files, oldest-first candidates via mtime."""
    base = root or data_root()
    files: list[tuple[Path, int, float]] = []
    if not base.exists():
        return files
    for dirpath, _dirnames, filenames in base.walk() if hasattr(base, "walk") else _walk(base):
        for name in filenames:
            path = Path(dirpath) / name
            # Never treat sibling user media as disposable from here.
            if "media_library" in path.parts:
                continue
            try:
                st = path.stat()
                files.append((path, st.st_size, st.st_mtime))
            except OSError:
                continue
    files.sort(key=lambda x: x[2])  # LRU-ish by mtime
    return files


def _walk(base: Path):
    import os

    for dirpath, dirnames, filenames in os.walk(base):
        yield dirpath, dirnames, filenames
