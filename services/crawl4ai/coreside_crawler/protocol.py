"""NDJSON protocol helpers. Stdout = protocol only; stderr = diagnostics."""

from __future__ import annotations

import json
import sys
from typing import Any

from . import PROTOCOL_VERSION

COMMANDS = frozenset(
    {
        "health",
        "runtime_info",
        "crawl_url",
        "crawl_urls",
        "discover_domain",
        "discover_from_page",
        "extract_media",
        "cancel",
        "cleanup_cache",
        "cache_stats",
        "resource_stats",
        "reset_browser",
        "shutdown",
    }
)

EVENT_TYPES = frozenset(
    {
        "accepted",
        "queued",
        "started",
        "robots_check",
        "browser_started",
        "loading_page",
        "extracting",
        "processing_media",
        "completed",
        "cancelled",
        "failed",
        "warning",
        "resource_pressure",
        "rate_limited",
        "progress",
    }
)


def emit(request_id: str | None, event_type: str, payload: dict[str, Any] | None = None) -> None:
    msg = {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "type": event_type,
        "payload": payload or {},
    }
    sys.stdout.write(json.dumps(msg, ensure_ascii=False, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def parse_line(line: str) -> dict[str, Any] | None:
    raw = line.strip()
    if not raw:
        return None
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return None
    if not isinstance(data, dict):
        return None
    return data


def validate_request(data: dict[str, Any]) -> tuple[str | None, str | None, dict[str, Any], str | None]:
    """Return (request_id, type, payload, error)."""
    version = str(data.get("protocolVersion", ""))
    if version != PROTOCOL_VERSION:
        return None, None, {}, f"unsupported protocolVersion: {version!r}"
    req_type = data.get("type")
    if not isinstance(req_type, str) or req_type not in COMMANDS:
        return data.get("requestId") if isinstance(data.get("requestId"), str) else None, None, {}, f"unknown type: {req_type!r}"
    request_id = data.get("requestId")
    if not isinstance(request_id, str) or not request_id.strip():
        return None, req_type, {}, "requestId is required"
    payload = data.get("payload")
    if payload is None:
        payload = {}
    if not isinstance(payload, dict):
        return request_id, req_type, {}, "payload must be an object"
    return request_id, req_type, payload, None
