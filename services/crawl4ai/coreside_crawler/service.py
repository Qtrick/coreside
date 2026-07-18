"""Stdio NDJSON service loop."""

from __future__ import annotations

import asyncio
import logging
import sys
from typing import Any

from . import PROTOCOL_VERSION
from .cache import cache_stats
from .cleanup import cleanup_cache
from .crawler_manager import CrawlerManager
from .discovery.domain_seeder import discover_domain
from .discovery.direct_url import discover_direct_urls
from .discovery.page_links import discover_from_links
from .discovery.ranking import rank_candidates
from .errors import CANCELLED, CrawlError, INVALID_REQUEST, ENGINE_UNAVAILABLE
from .protocol import emit, parse_line, validate_request

log = logging.getLogger("coreside_crawler")


class ResearchService:
    def __init__(self) -> None:
        self.manager = CrawlerManager()
        self._running = True
        self._tasks: dict[str, asyncio.Task] = {}

    async def handle(self, request_id: str, req_type: str, payload: dict[str, Any]) -> None:
        emit(request_id, "accepted", {"type": req_type})
        try:
            if req_type == "health":
                info = self.manager.runtime_info()
                emit(request_id, "completed", {"ok": True, "runtime": info, "protocolVersion": PROTOCOL_VERSION})
                return
            if req_type == "runtime_info":
                emit(request_id, "completed", self.manager.runtime_info())
                return
            if req_type == "resource_stats":
                emit(request_id, "completed", self.manager.stats())
                return
            if req_type == "cache_stats":
                emit(request_id, "completed", cache_stats())
                return
            if req_type == "cleanup_cache":
                emit(request_id, "completed", cleanup_cache())
                return
            if req_type == "reset_browser":
                await self.manager.reset_browser()
                emit(request_id, "completed", {"ok": True})
                return
            if req_type == "cancel":
                target = payload.get("targetRequestId") or payload.get("requestId") or request_id
                ok = self.manager.cancel(str(target))
                task = self._tasks.get(str(target))
                if task and not task.done():
                    task.cancel()
                emit(request_id, "completed", {"cancelled": ok, "targetRequestId": target})
                return
            if req_type == "shutdown":
                self._running = False
                await self.manager.shutdown()
                emit(request_id, "completed", {"ok": True})
                return

            if req_type in {"crawl_url", "extract_media"}:
                await self._crawl_one(request_id, payload, media=req_type == "extract_media")
                return
            if req_type == "crawl_urls":
                await self._crawl_many(request_id, payload)
                return
            if req_type == "discover_domain":
                await self._discover_domain(request_id, payload)
                return
            if req_type == "discover_from_page":
                await self._discover_from_page(request_id, payload)
                return

            raise CrawlError(INVALID_REQUEST, f"unhandled type: {req_type}")
        except asyncio.CancelledError:
            emit(request_id, "cancelled", {"category": CANCELLED, "message": "cancelled"})
            raise
        except CrawlError as e:
            emit(request_id, "failed", e.to_payload())
        except Exception as e:  # pragma: no cover
            log.exception("request failed")
            emit(request_id, "failed", {"category": ENGINE_UNAVAILABLE, "message": str(e)})
        finally:
            if req_type not in {"cancel", "shutdown"}:
                self.manager.clear_cancel(request_id)

    def _progress(self, request_id: str):
        def cb(stage: str, payload: dict[str, Any] | None = None) -> None:
            emit(request_id, stage if stage in {
                "robots_check", "browser_started", "loading_page", "extracting",
                "processing_media", "resource_pressure", "rate_limited", "warning", "progress",
            } else "progress", payload or {"stage": stage})

        return cb

    async def _crawl_one(self, request_id: str, payload: dict[str, Any], *, media: bool) -> None:
        url = payload.get("url")
        if not isinstance(url, str) or not url.strip():
            raise CrawlError(INVALID_REQUEST, "url is required")
        profile = payload.get("profile") or ("media" if media else "text")
        if profile not in {"text", "media", "dynamic"}:
            profile = "text"
        if "resourceProfile" in payload:
            self.manager.set_resource_profile(str(payload["resourceProfile"]))
        self.manager.register_cancel(request_id)
        emit(request_id, "started", {"url": url, "profile": profile})
        result = await self.manager.crawl_url(
            url,
            request_id=request_id,
            profile=profile,  # type: ignore[arg-type]
            cache_mode=str(payload.get("cacheMode") or "standard"),
            allow_loopback_for_tests=bool(payload.get("allowLoopbackForTests")),
            include_media=media or profile == "media",
            progress_cb=self._progress(request_id),
        )
        emit(request_id, "completed", result)

    async def _crawl_many(self, request_id: str, payload: dict[str, Any]) -> None:
        urls = payload.get("urls")
        if not isinstance(urls, list) or not urls:
            raise CrawlError(INVALID_REQUEST, "urls must be a non-empty array")
        profile = payload.get("profile") or "text"
        if profile not in {"text", "media", "dynamic"}:
            profile = "text"
        if "resourceProfile" in payload:
            self.manager.set_resource_profile(str(payload["resourceProfile"]))
        self.manager.register_cancel(request_id)
        emit(request_id, "started", {"count": len(urls)})
        result = await self.manager.crawl_urls(
            [str(u) for u in urls],
            request_id=request_id,
            profile=profile,  # type: ignore[arg-type]
            cache_mode=str(payload.get("cacheMode") or "standard"),
            allow_loopback_for_tests=bool(payload.get("allowLoopbackForTests")),
            progress_cb=self._progress(request_id),
        )
        emit(request_id, "completed", result)

    async def _discover_domain(self, request_id: str, payload: dict[str, Any]) -> None:
        domain = payload.get("domain") or payload.get("url")
        if not isinstance(domain, str) or not domain.strip():
            raise CrawlError(INVALID_REQUEST, "domain or url is required")
        query = str(payload.get("query") or "")
        limit = min(int(payload.get("limit") or 20), 20)
        self.manager.register_cancel(request_id)
        emit(request_id, "started", {"domain": domain})

        async def crawl_fn(u: str):
            return await self.manager.crawl_url(
                u,
                request_id=request_id,
                profile="text",
                allow_loopback_for_tests=bool(payload.get("allowLoopbackForTests")),
                progress_cb=self._progress(request_id),
            )

        candidates = await discover_domain(
            domain,
            query=query,
            limit=limit,
            crawl_fn=crawl_fn,
            allow_loopback_for_tests=bool(payload.get("allowLoopbackForTests")),
        )
        emit(request_id, "completed", {"candidates": candidates, "discoveryOnly": True})

    async def _discover_from_page(self, request_id: str, payload: dict[str, Any]) -> None:
        url = payload.get("url")
        if not isinstance(url, str):
            raise CrawlError(INVALID_REQUEST, "url is required")
        query = str(payload.get("query") or "")
        self.manager.register_cancel(request_id)
        page = await self.manager.crawl_url(
            url,
            request_id=request_id,
            profile="text",
            allow_loopback_for_tests=bool(payload.get("allowLoopbackForTests")),
            progress_cb=self._progress(request_id),
        )
        links = page.get("links") or []
        candidates = rank_candidates(
            discover_from_links(links, query=query) + discover_direct_urls([url]),
            query=query,
            limit=min(int(payload.get("limit") or 20), 20),
        )
        emit(request_id, "completed", {"candidates": candidates, "sourcePage": page.get("source")})

    async def run(self) -> None:
        loop = asyncio.get_running_loop()
        log.info("Coreside crawler sidecar ready (protocol=%s)", PROTOCOL_VERSION)
        # Startup cleanup
        try:
            cleanup_cache()
        except Exception as e:
            log.warning("startup cleanup: %s", e)

        while self._running:
            line = await loop.run_in_executor(None, sys.stdin.readline)
            if line == "":
                break
            data = parse_line(line)
            if data is None:
                emit(None, "warning", {"message": "malformed protocol message ignored"})
                continue
            request_id, req_type, payload, err = validate_request(data)
            if err:
                emit(request_id, "failed", {"category": INVALID_REQUEST, "message": err})
                continue
            assert request_id and req_type
            task = asyncio.create_task(self.handle(request_id, req_type, payload))
            self._tasks[request_id] = task

            def _done(t: asyncio.Task, rid: str = request_id) -> None:
                self._tasks.pop(rid, None)
                try:
                    t.result()
                except (asyncio.CancelledError, Exception):
                    pass

            task.add_done_callback(_done)

        await self.manager.shutdown()


def main() -> None:
    logging.basicConfig(stream=sys.stderr, level=logging.INFO, format="[coreside-crawler] %(message)s")
    try:
        asyncio.run(ResearchService().run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
