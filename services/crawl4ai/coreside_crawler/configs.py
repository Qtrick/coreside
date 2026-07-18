"""Trusted BrowserConfig / CrawlerRunConfig builders. AI cannot supply arbitrary configs."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from crawl4ai import BrowserConfig, CacheMode, CrawlerRunConfig
from crawl4ai.content_filter_strategy import PruningContentFilter
from crawl4ai.markdown_generation_strategy import DefaultMarkdownGenerator

from . import CORESIDE_USER_AGENT

CrawlProfile = Literal["text", "media", "dynamic"]
ResourceProfile = Literal["eco", "balanced", "performance"]
CoresideCacheMode = Literal["fresh", "standard", "offline"]

# Hard ceilings — not overridable by AI or generated settings.
HARD_MAX_ACTIVE_PAGES = 4
HARD_MAX_PAGES_PER_REQUEST = 10
HARD_MAX_CRAWL_DEPTH = 2
HARD_MAX_EXTRACTED_CHARS = 24_000
HARD_MAX_MARKDOWN_CHARS = 16_000
HARD_MAX_MEDIA_RESULTS = 40
HARD_MAX_SCREENSHOTS = 0  # default disabled
HARD_MAX_QUEUED = 20
HARD_PAGE_TIMEOUT_MS = 45_000
IDLE_BROWSER_SECONDS = 180  # 3 minutes


@dataclass(frozen=True)
class ResourceLimits:
    name: ResourceProfile
    global_concurrency: int
    max_pages: int
    domain_concurrency: int
    memory_threshold_percent: float
    idle_browser_seconds: int


RESOURCE_PROFILES: dict[ResourceProfile, ResourceLimits] = {
    "eco": ResourceLimits("eco", 1, 1, 1, 62.0, 90),
    "balanced": ResourceLimits("balanced", 2, 2, 1, 70.0, IDLE_BROWSER_SECONDS),
    "performance": ResourceLimits("performance", 3, 4, 2, 75.0, 300),
}


def resolve_resource_profile(name: str | None) -> ResourceLimits:
    key = (name or "balanced").lower()
    if key not in RESOURCE_PROFILES:
        key = "balanced"
    return RESOURCE_PROFILES[key]  # type: ignore[index]


def map_cache_mode(mode: CoresideCacheMode | str | None) -> CacheMode:
    m = (mode or "standard").lower()
    if m == "fresh":
        return CacheMode.BYPASS
    if m == "offline":
        return CacheMode.READ_ONLY
    return CacheMode.ENABLED


def build_browser_config(resource: ResourceLimits, crawl_profile: CrawlProfile) -> BrowserConfig:
    text_mode = crawl_profile == "text"
    return BrowserConfig(
        headless=True,
        verbose=False,
        text_mode=text_mode,
        light_mode=text_mode or resource.name == "eco",
        java_script_enabled=crawl_profile != "text",
        enable_stealth=False,  # never
        accept_downloads=False,
        user_agent=CORESIDE_USER_AGENT,
        user_agent_mode="",
        avoid_ads=True,
        memory_saving_mode=resource.name == "eco",
        extra_args=["--disable-dev-shm-usage", "--no-sandbox"] if resource.name == "eco" else None,
    )


def build_crawler_config(
    crawl_profile: CrawlProfile,
    *,
    cache_mode: CoresideCacheMode | str | None = "standard",
    page_timeout_ms: int | None = None,
) -> CrawlerRunConfig:
    timeout = min(page_timeout_ms or HARD_PAGE_TIMEOUT_MS, HARD_PAGE_TIMEOUT_MS)
    md_gen = DefaultMarkdownGenerator(
        content_filter=PruningContentFilter(threshold=0.45, threshold_type="fixed")
    )
    exclude_images = crawl_profile == "text"
    return CrawlerRunConfig(
        cache_mode=map_cache_mode(cache_mode),
        check_robots_txt=True,  # always — AI cannot disable
        user_agent=CORESIDE_USER_AGENT,
        page_timeout=timeout,
        screenshot=False,
        pdf=False,
        capture_mhtml=False,
        exclude_all_images=exclude_images,
        exclude_external_images=exclude_images,
        word_count_threshold=10,
        markdown_generator=md_gen,
        excluded_tags=["nav", "footer", "aside", "script", "style", "noscript"],
        verbose=False,
        wait_for=None if crawl_profile != "dynamic" else "css:body",
        scan_full_page=crawl_profile == "dynamic",
        process_iframes=False,
        remove_overlay_elements=crawl_profile == "dynamic",
        max_retries=0,  # Coreside owns retry/backoff
    )
