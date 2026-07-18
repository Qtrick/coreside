"""Error categories for crawl / research failures."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class CrawlError(Exception):
    category: str
    message: str
    details: dict[str, Any] | None = None

    def to_payload(self) -> dict[str, Any]:
        out: dict[str, Any] = {"category": self.category, "message": self.message}
        if self.details:
            out["details"] = self.details
        return out


ROBOTS_DISALLOWED = "robots_disallowed"
ANTI_BOT_BLOCKED = "anti_bot_blocked"
CAPTCHA_REQUIRED = "captcha_required"
LOGIN_REQUIRED = "login_required"
RATE_LIMITED = "rate_limited"
NAVIGATION_TIMEOUT = "navigation_timeout"
DYNAMIC_CONTENT_UNAVAILABLE = "dynamic_content_unavailable"
UNSUPPORTED_CONTENT = "unsupported_content"
ACCESS_DENIED = "access_denied"
SSRF_BLOCKED = "ssrf_blocked"
LOW_DISK_SPACE = "low_disk_space"
RESOURCE_PRESSURE = "resource_pressure"
CANCELLED = "cancelled"
INVALID_REQUEST = "invalid_request"
ENGINE_UNAVAILABLE = "engine_unavailable"
UNKNOWN = "unknown"


def classify_failure(message: str, status_code: int | None = None) -> str:
    text = (message or "").lower()
    if status_code == 429 or "429" in text or "rate limit" in text:
        return RATE_LIMITED
    if status_code == 503:
        return RATE_LIMITED
    if status_code in (401, 403) or "access denied" in text or "forbidden" in text:
        return ACCESS_DENIED
    if "robots" in text and ("disallow" in text or "blocked" in text):
        return ROBOTS_DISALLOWED
    if "captcha" in text or "cf-challenge" in text or "cloudflare" in text:
        return CAPTCHA_REQUIRED if "captcha" in text else ANTI_BOT_BLOCKED
    if "login" in text or "sign in" in text or "authenticate" in text:
        return LOGIN_REQUIRED
    if "timeout" in text or "timed out" in text:
        return NAVIGATION_TIMEOUT
    if "ssrf" in text or "private" in text or "blocked host" in text:
        return SSRF_BLOCKED
    return UNKNOWN
