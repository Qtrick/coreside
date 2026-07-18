"""Dispatchers and rate limiters from trusted resource profiles only."""

from __future__ import annotations

from crawl4ai import MemoryAdaptiveDispatcher, RateLimiter, SemaphoreDispatcher

from .configs import ResourceLimits


def build_rate_limiter() -> RateLimiter:
    # Polite defaults: 2–5s domain delay, max backoff 60s, few retries.
    return RateLimiter(
        base_delay=(2.0, 5.0),
        max_delay=60.0,
        max_retries=2,
        rate_limit_codes=[429, 503],
    )


def build_dispatcher(limits: ResourceLimits, *, prefer_memory: bool = True):
    rate = build_rate_limiter()
    permit = min(limits.global_concurrency, limits.max_pages)
    if prefer_memory:
        try:
            return MemoryAdaptiveDispatcher(
                memory_threshold_percent=limits.memory_threshold_percent,
                critical_threshold_percent=min(limits.memory_threshold_percent + 8, 90.0),
                recovery_threshold_percent=max(limits.memory_threshold_percent - 8, 50.0),
                check_interval=1.0,
                max_session_permit=permit,
                fairness_timeout=120.0,
                memory_wait_timeout=90.0,
                rate_limiter=rate,
            )
        except Exception:
            pass
    return SemaphoreDispatcher(max_session_permit=permit, rate_limiter=rate)
