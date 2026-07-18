"""Single managed AsyncWebCrawler lifecycle with idle shutdown."""

from __future__ import annotations

import asyncio
import logging
import time
from typing import Any
from urllib.parse import urlparse

from crawl4ai import AsyncWebCrawler

from . import PINNED_CRAWL4AI
from .compliance import DomainPolicy
from .configs import (
    HARD_MAX_ACTIVE_PAGES,
    HARD_MAX_PAGES_PER_REQUEST,
    CrawlProfile,
    ResourceLimits,
    build_browser_config,
    build_crawler_config,
    resolve_resource_profile,
)
from .dispatchers import build_dispatcher
from .errors import (
    CANCELLED,
    CrawlError,
    ROBOTS_DISALLOWED,
    classify_failure,
)
from .media import normalize_media
from .normalization import detect_anti_bot, extract_links, normalize_source
from .resources import check_can_start_crawl, ensure_crawl4ai_env, resource_stats
from .security import validate_public_http_url

log = logging.getLogger("coreside_crawler")


class CrawlerManager:
    def __init__(self) -> None:
        self._crawler: AsyncWebCrawler | None = None
        self._lock = asyncio.Lock()
        self._active_pages = 0
        self._queued = 0
        self._last_used = 0.0
        self._idle_task: asyncio.Task | None = None
        self._cancel_flags: dict[str, asyncio.Event] = {}
        self._resource = resolve_resource_profile("balanced")
        self._domain_policy = DomainPolicy()
        self._browser_started = False
        self._crawl_profile: CrawlProfile | None = None
        self._browser_resource_name: str | None = None
        ensure_crawl4ai_env()

    @property
    def resource(self) -> ResourceLimits:
        return self._resource

    def set_resource_profile(self, name: str) -> None:
        nxt = resolve_resource_profile(name)
        if nxt.name != self._resource.name:
            self._resource = nxt
            # Force browser rebuild so eco/performance args take effect.
            self._browser_resource_name = None

    def register_cancel(self, request_id: str) -> asyncio.Event:
        ev = asyncio.Event()
        self._cancel_flags[request_id] = ev
        return ev

    def clear_cancel(self, request_id: str | None) -> None:
        if request_id:
            self._cancel_flags.pop(request_id, None)

    def cancel(self, request_id: str) -> bool:
        ev = self._cancel_flags.get(request_id)
        if ev:
            ev.set()
            return True
        return False

    def _cancelled(self, request_id: str | None) -> bool:
        if not request_id:
            return False
        ev = self._cancel_flags.get(request_id)
        return bool(ev and ev.is_set())

    async def _stop_crawler_locked(self) -> None:
        if self._crawler is not None:
            try:
                await self._crawler.__aexit__(None, None, None)
            except Exception as e:  # pragma: no cover
                log.warning("browser shutdown error: %s", e)
            self._crawler = None
            self._browser_started = False
            self._crawl_profile = None
            self._browser_resource_name = None
            log.info("AsyncWebCrawler stopped")

    async def ensure_crawler(self, crawl_profile: CrawlProfile = "text") -> AsyncWebCrawler:
        async with self._lock:
            needs_rebuild = (
                self._crawler is None
                or self._crawl_profile != crawl_profile
                or self._browser_resource_name != self._resource.name
            )
            if needs_rebuild and self._crawler is not None:
                await self._stop_crawler_locked()
            if self._crawler is None:
                browser = build_browser_config(self._resource, crawl_profile)
                self._crawler = AsyncWebCrawler(config=browser)
                await self._crawler.__aenter__()
                self._browser_started = True
                self._crawl_profile = crawl_profile
                self._browser_resource_name = self._resource.name
                log.info(
                    "AsyncWebCrawler started (crawl=%s resource=%s)",
                    crawl_profile,
                    self._resource.name,
                )
            self._last_used = time.time()
            self._arm_idle()
            return self._crawler

    def _arm_idle(self) -> None:
        if self._idle_task and not self._idle_task.done():
            self._idle_task.cancel()
        self._idle_task = asyncio.create_task(self._idle_watch())

    async def _idle_watch(self) -> None:
        try:
            await asyncio.sleep(self._resource.idle_browser_seconds)
            if self._active_pages == 0 and time.time() - self._last_used >= self._resource.idle_browser_seconds - 1:
                await self.reset_browser()
        except asyncio.CancelledError:
            return

    async def reset_browser(self) -> None:
        async with self._lock:
            await self._stop_crawler_locked()

    async def shutdown(self) -> None:
        for ev in self._cancel_flags.values():
            ev.set()
        if self._idle_task:
            self._idle_task.cancel()
        await self.reset_browser()

    def runtime_info(self) -> dict[str, Any]:
        import crawl4ai

        ver = getattr(crawl4ai, "__version__", None)
        if hasattr(ver, "__file__"):
            try:
                from crawl4ai.__version__ import __version__ as v

                ver = v
            except Exception:
                ver = PINNED_CRAWL4AI
        return {
            "sidecarVersion": "0.1.0",
            "crawl4aiVersion": str(ver or PINNED_CRAWL4AI),
            "pinnedCrawl4ai": PINNED_CRAWL4AI,
            "browserStarted": self._browser_started,
            "activePages": self._active_pages,
            "resourceProfile": self._resource.name,
            "idleBrowserSeconds": self._resource.idle_browser_seconds,
        }

    def stats(self) -> dict[str, Any]:
        return resource_stats(self._resource, active_pages=self._active_pages, queued=self._queued)

    async def crawl_url(
        self,
        url: str,
        *,
        request_id: str | None = None,
        profile: CrawlProfile = "text",
        cache_mode: str = "standard",
        allow_loopback_for_tests: bool = False,
        include_media: bool = False,
        progress_cb=None,
    ) -> dict[str, Any]:
        validate_public_http_url(url, allow_loopback_for_tests=allow_loopback_for_tests)
        check_can_start_crawl(self._resource)
        domain = urlparse(url).hostname or ""
        ok, reason = self._domain_policy.can_request(domain)
        if not ok:
            raise CrawlError("rate_limited", f"Domain temporarily unavailable: {reason}", {"domain": domain})

        if self._cancelled(request_id):
            raise CrawlError(CANCELLED, "cancelled")

        if progress_cb:
            progress_cb("robots_check", {"message": "Checking robots.txt", "url": url})

        page_cap = min(self._resource.max_pages, HARD_MAX_ACTIVE_PAGES)
        if self._active_pages >= page_cap:
            raise CrawlError(
                "resource_pressure",
                "Active browser page limit reached",
                {"activePages": self._active_pages, "max": page_cap},
            )
        self._active_pages += 1
        try:
            crawler = await self.ensure_crawler(profile)
            if progress_cb and self._browser_started:
                progress_cb("browser_started", {"message": "Browser ready"})
            if progress_cb:
                progress_cb("loading_page", {"message": "Loading source", "url": url})

            config = build_crawler_config(profile, cache_mode=cache_mode)
            if self._cancelled(request_id):
                raise CrawlError(CANCELLED, "cancelled")

            result = await crawler.arun(url=url, config=config)

            if self._cancelled(request_id):
                raise CrawlError(CANCELLED, "cancelled")

            if progress_cb:
                progress_cb("extracting", {"message": "Extracting readable content"})

            # robots / failure detection
            err_msg = getattr(result, "error_message", None) or ""
            if not getattr(result, "success", False):
                cat = classify_failure(str(err_msg), getattr(result, "status_code", None))
                if "robot" in str(err_msg).lower():
                    cat = ROBOTS_DISALLOWED
                self._domain_policy.record_failure(domain, status_code=getattr(result, "status_code", None))
                raise CrawlError(cat, str(err_msg) or "Crawl failed", {"url": url})

            source = normalize_source(result, profile=profile)
            # Redirect SSRF: re-validate final URL after the browser follows redirects.
            final_url = source.get("finalUrl") or source.get("url") or url
            validate_public_http_url(
                str(final_url), allow_loopback_for_tests=allow_loopback_for_tests
            )
            anti = detect_anti_bot(source.get("markdown") or "")
            if anti:
                source["success"] = False
                source["failureCategory"] = anti
                raise CrawlError(anti, "Source appears protected or inaccessible", {"url": url})

            links = extract_links(result, base_url=source.get("finalUrl") or url)
            media_payload: dict[str, Any] = {"images": [], "videos": [], "audios": []}
            if include_media or profile == "media":
                if progress_cb:
                    progress_cb("processing_media", {"message": "Processing media references"})
                media_payload = normalize_media(result, include_audio=False)

            self._domain_policy.record_success(domain)
            self._last_used = time.time()
            return {
                "source": source,
                "content": {
                    "markdown": source.get("markdown"),
                    "excerpt": source.get("excerpt"),
                    "truncated": True,
                },
                "links": links,
                "media": media_payload,
                "complianceNotice": "Coreside respects site restrictions and may be unable to access some sources.",
            }
        finally:
            self._active_pages = max(0, self._active_pages - 1)

    async def crawl_urls(
        self,
        urls: list[str],
        *,
        request_id: str | None = None,
        profile: CrawlProfile = "text",
        cache_mode: str = "standard",
        allow_loopback_for_tests: bool = False,
        progress_cb=None,
    ) -> dict[str, Any]:
        capped = urls[:HARD_MAX_PAGES_PER_REQUEST]
        for u in capped:
            validate_public_http_url(u, allow_loopback_for_tests=allow_loopback_for_tests)
        check_can_start_crawl(self._resource)
        if self._cancelled(request_id):
            raise CrawlError(CANCELLED, "cancelled")

        # Prefer MemoryAdaptiveDispatcher for multi-URL; fall back to sequential on failure.
        try:
            return await self._crawl_urls_dispatched(
                capped,
                request_id=request_id,
                profile=profile,
                cache_mode=cache_mode,
                allow_loopback_for_tests=allow_loopback_for_tests,
                progress_cb=progress_cb,
            )
        except Exception as e:
            log.warning("dispatcher batch failed (%s); falling back to sequential", e)

        results = []
        errors = []
        for u in capped:
            if self._cancelled(request_id):
                raise CrawlError(CANCELLED, "cancelled")
            try:
                item = await self.crawl_url(
                    u,
                    request_id=request_id,
                    profile=profile,
                    cache_mode=cache_mode,
                    allow_loopback_for_tests=allow_loopback_for_tests,
                    include_media=profile == "media",
                    progress_cb=progress_cb,
                )
                results.append(item)
            except CrawlError as e:
                errors.append({"url": u, **e.to_payload()})
        return {"results": results, "errors": errors, "requested": len(capped)}

    async def _crawl_urls_dispatched(
        self,
        urls: list[str],
        *,
        request_id: str | None,
        profile: CrawlProfile,
        cache_mode: str,
        allow_loopback_for_tests: bool,
        progress_cb,
    ) -> dict[str, Any]:
        crawler = await self.ensure_crawler(profile)
        config = build_crawler_config(profile, cache_mode=cache_mode)
        dispatcher = build_dispatcher(self._resource, prefer_memory=True)
        if progress_cb:
            progress_cb("started", {"message": "Crawling sources", "count": len(urls)})
        self._active_pages += min(len(urls), self._resource.max_pages)
        try:
            if self._cancelled(request_id):
                raise CrawlError(CANCELLED, "cancelled")
            batch = await crawler.arun_many(urls=urls, config=config, dispatcher=dispatcher)
            if not isinstance(batch, list):
                batch = [batch]
            results = []
            errors = []
            for result in batch:
                if self._cancelled(request_id):
                    raise CrawlError(CANCELLED, "cancelled")
                url = getattr(result, "url", "") or ""
                try:
                    if not getattr(result, "success", False):
                        err_msg = getattr(result, "error_message", None) or "Crawl failed"
                        cat = classify_failure(str(err_msg), getattr(result, "status_code", None))
                        if "robot" in str(err_msg).lower():
                            cat = ROBOTS_DISALLOWED
                        raise CrawlError(cat, str(err_msg), {"url": url})
                    source = normalize_source(result, profile=profile)
                    final_url = source.get("finalUrl") or source.get("url") or url
                    validate_public_http_url(
                        str(final_url), allow_loopback_for_tests=allow_loopback_for_tests
                    )
                    anti = detect_anti_bot(source.get("markdown") or "")
                    if anti:
                        raise CrawlError(anti, "Source appears protected or inaccessible", {"url": url})
                    media_payload = {"images": [], "videos": [], "audios": []}
                    if profile == "media":
                        media_payload = normalize_media(result, include_audio=False)
                    results.append(
                        {
                            "source": source,
                            "content": {
                                "markdown": source.get("markdown"),
                                "excerpt": source.get("excerpt"),
                                "truncated": True,
                            },
                            "links": extract_links(result, base_url=str(final_url)),
                            "media": media_payload,
                        }
                    )
                except CrawlError as e:
                    errors.append({"url": url, **e.to_payload()})
            return {"results": results, "errors": errors, "requested": len(urls)}
        finally:
            self._active_pages = max(0, self._active_pages - min(len(urls), self._resource.max_pages))
            self._last_used = time.time()
