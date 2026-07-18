# Coreside Crawl4AI sidecar

Pinned dependency: **crawl4ai==0.9.2** (verified 2026-07-17).

This is an internal Coreside infrastructure service. Consumers see **Web Research** in the app, not a Crawl4AI admin console.

## Setup

From the repository root:

```bash
npm run crawl4ai:setup
npm run crawl4ai:doctor
npm run crawl4ai:smoke
npm run crawl4ai:test
```

Uses a project-local virtualenv at `services/crawl4ai/.venv` (not global Python).

Preferred Python: **3.11–3.13**. Avoid bleeding-edge 3.14 until Crawl4AI confirms support.

Managed data root (via env):

- `CRAWL4_AI_BASE_DIRECTORY` → Crawl4AI cache parent
- `CORESIDE_CRAWLER_DATA_ROOT` → Coreside cleanup root

## Run sidecar (stdio NDJSON)

```bash
services/crawl4ai/.venv/bin/python -m coreside_crawler
```

Stdout is protocol messages only. Diagnostics go to stderr.

## Protocol

See `coreside_crawler/protocol.py`. Commands include `health`, `crawl_url`, `crawl_urls`, `cancel`, `shutdown`, discovery, cache, and resource stats.

## Notes

- robots.txt is always checked (`check_robots_txt=True`).
- Stealth / undetected / proxies are disabled.
- LLM extraction is not used by default.
- Crawl4AI is a crawler, not a global search index — discovery is a separate stage.
