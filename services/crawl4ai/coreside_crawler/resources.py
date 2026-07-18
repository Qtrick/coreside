"""Resource / disk / memory helpers."""

from __future__ import annotations

import os
import shutil
from pathlib import Path
from typing import Any

try:
    import psutil
except ImportError:  # pragma: no cover
    psutil = None  # type: ignore

from .configs import HARD_MAX_QUEUED, ResourceLimits
from .errors import CrawlError, LOW_DISK_SPACE, RESOURCE_PRESSURE


def data_root() -> Path:
    override = os.environ.get("CORESIDE_CRAWLER_DATA_ROOT")
    if override:
        p = Path(override)
    else:
        base = os.environ.get("CRAWL4_AI_BASE_DIRECTORY")
        if base:
            p = Path(base) / ".crawl4ai"
        else:
            p = Path.home() / ".coreside" / "crawler"
    p.mkdir(parents=True, exist_ok=True)
    return p


def ensure_crawl4ai_env(root: Path | None = None) -> Path:
    """Point Crawl4AI cache at the managed root via CRAWL4_AI_BASE_DIRECTORY."""
    root = root or data_root().parent
    root.mkdir(parents=True, exist_ok=True)
    os.environ["CRAWL4_AI_BASE_DIRECTORY"] = str(root)
    return root


def memory_percent() -> float | None:
    if psutil is None:
        return None
    return float(psutil.virtual_memory().percent)


def cpu_percent() -> float | None:
    if psutil is None:
        return None
    return float(psutil.cpu_percent(interval=0.05))


def disk_usage(path: Path | None = None) -> dict[str, int]:
    target = path or data_root()
    usage = shutil.disk_usage(target)
    return {
        "totalBytes": usage.total,
        "usedBytes": usage.used,
        "freeBytes": usage.free,
    }


def cache_size_bytes(root: Path | None = None) -> int:
    base = root or data_root()
    total = 0
    if not base.exists():
        return 0
    for dirpath, _dirnames, filenames in os.walk(base):
        for name in filenames:
            try:
                total += (Path(dirpath) / name).stat().st_size
            except OSError:
                continue
    return total


# Absolute hard stop — only when the machine is critically saturated.
HARD_MEMORY_BLOCK_PERCENT = 90.0


def check_can_start_crawl(limits: ResourceLimits, *, estimated_bytes: int = 5_000_000) -> None:
    mem = memory_percent()
    # Profile thresholds drive dispatcher slowing; only hard-block near OOM.
    if mem is not None and mem >= HARD_MEMORY_BLOCK_PERCENT:
        raise CrawlError(
            RESOURCE_PRESSURE,
            "Coreside slowed web research to protect system performance.",
            {
                "memoryPercent": mem,
                "threshold": HARD_MEMORY_BLOCK_PERCENT,
                "profileThreshold": limits.memory_threshold_percent,
            },
        )
    disk = disk_usage()
    # Require ~200MB free or estimated size, whichever larger.
    need = max(estimated_bytes, 200_000_000)
    if disk["freeBytes"] < need:
        raise CrawlError(
            LOW_DISK_SPACE,
            "Not enough free disk space for web research.",
            {"freeBytes": disk["freeBytes"], "requiredBytes": need},
        )


def under_profile_pressure(limits: ResourceLimits) -> bool:
    mem = memory_percent()
    return mem is not None and mem >= limits.memory_threshold_percent


def resource_stats(limits: ResourceLimits, *, active_pages: int, queued: int) -> dict[str, Any]:
    return {
        "profile": limits.name,
        "memoryPercent": memory_percent(),
        "cpuPercent": cpu_percent(),
        "disk": disk_usage(),
        "cacheBytes": cache_size_bytes(),
        "activePages": active_pages,
        "queued": queued,
        "maxPages": limits.max_pages,
        "maxQueued": HARD_MAX_QUEUED,
        "memoryThresholdPercent": limits.memory_threshold_percent,
    }
