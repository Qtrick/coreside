# Crawl4AI Packaged Runtime

**Product:** Coreside  
**Access date:** 2026-08-03  
**Status:** Partial

## Data root

Resolved through `AppPaths.crawler` (canonical product data). No repository `.coreside/crawler` fallback in release.

## Service root

Priority:

1. `CORESIDE_CRAWLER_SERVICE_ROOT`
2. Managed `AppPaths.crawler/service`
3. Debug-only repository `services/crawl4ai` when present

Release builds report `NeedsSetup` when the managed service is absent instead of depending on `CARGO_MANIFEST_DIR`.

## Remaining work

- Bundle or verified install of service sources into managed data
- Resource hash verification
- Packaged doctor smoke without repository
