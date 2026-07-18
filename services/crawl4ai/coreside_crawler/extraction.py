"""Thin extraction helpers (deterministic; no LLMExtractionStrategy by default)."""

from __future__ import annotations

from .configs import HARD_MAX_EXTRACTED_CHARS, HARD_MAX_MARKDOWN_CHARS
from .normalization import _clip


def bound_text_for_model(markdown: str, *, query: str = "") -> dict[str, str | bool]:
    """Select bounded excerpts before sending to the Coreside AI agent."""
    text = markdown or ""
    truncated = len(text) > HARD_MAX_MARKDOWN_CHARS
    clipped = _clip(text, HARD_MAX_MARKDOWN_CHARS)
    # Prefer query-overlapping windows when present.
    q = [t for t in (query or "").lower().split() if len(t) > 2]
    excerpt = clipped
    if q and len(clipped) > HARD_MAX_EXTRACTED_CHARS:
        lower = clipped.lower()
        best = 0
        best_score = -1
        window = HARD_MAX_EXTRACTED_CHARS
        step = max(500, window // 4)
        for i in range(0, max(1, len(lower) - window), step):
            chunk = lower[i : i + window]
            score = sum(1 for t in q if t in chunk)
            if score > best_score:
                best_score = score
                best = i
        excerpt = clipped[best : best + window]
    else:
        excerpt = _clip(clipped, HARD_MAX_EXTRACTED_CHARS)
    return {"markdown": clipped, "excerpt": excerpt, "truncated": truncated}
