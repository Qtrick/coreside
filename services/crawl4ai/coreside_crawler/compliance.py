"""Compliance helpers — robots is enforced by Crawl4AI; domain policy layer."""

from __future__ import annotations

from dataclasses import dataclass, field
from time import time
from typing import Any


@dataclass
class DomainState:
    domain: str
    last_request_at: float = 0.0
    cooldown_until: float = 0.0
    consecutive_failures: int = 0
    recent_429: int = 0
    recent_503: int = 0
    robots_status: str = "unknown"
    notes: list[str] = field(default_factory=list)


class DomainPolicy:
    """In-memory domain fairness + cooldown. Rust also persists domain_crawl_state."""

    def __init__(self) -> None:
        self._domains: dict[str, DomainState] = {}
        self.blocked: set[str] = set()

    def get(self, domain: str) -> DomainState:
        if domain not in self._domains:
            self._domains[domain] = DomainState(domain=domain)
        return self._domains[domain]

    def can_request(self, domain: str) -> tuple[bool, str | None]:
        if domain in self.blocked:
            return False, "domain_blocked"
        state = self.get(domain)
        now = time()
        if state.cooldown_until > now:
            return False, "domain_cooldown"
        return True, None

    def record_success(self, domain: str) -> None:
        state = self.get(domain)
        state.last_request_at = time()
        state.consecutive_failures = 0

    def record_failure(self, domain: str, *, status_code: int | None = None) -> None:
        state = self.get(domain)
        state.last_request_at = time()
        state.consecutive_failures += 1
        if status_code == 429:
            state.recent_429 += 1
            state.cooldown_until = time() + min(60 * state.recent_429, 300)
        elif status_code == 503:
            state.recent_503 += 1
            state.cooldown_until = time() + min(30 * state.recent_503, 180)
        elif state.consecutive_failures >= 3:
            state.cooldown_until = time() + 60

    def snapshot(self) -> list[dict[str, Any]]:
        now = time()
        return [
            {
                "domain": s.domain,
                "cooldownRemaining": max(0, s.cooldown_until - now),
                "consecutiveFailures": s.consecutive_failures,
                "recent429": s.recent_429,
                "recent503": s.recent_503,
                "robotsStatus": s.robots_status,
            }
            for s in self._domains.values()
        ]


COMPLIANCE_NOTICE = (
    "Coreside respects site restrictions and may be unable to access some sources."
)
