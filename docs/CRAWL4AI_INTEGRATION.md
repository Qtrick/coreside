# Crawl4AI integration

Coreside embeds [Crawl4AI](https://github.com/unclecode/crawl4ai) **0.9.2** as a managed local sidecar. Users see **Web Research** in the product UI — not a Crawl4AI admin console.

License: **Apache-2.0** (see [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)).

## Layout

```text
services/crawl4ai/           Python package + project venv (.venv)
  coreside_crawler/          Coreside protocol, compliance, cache, discovery
scripts/crawl4ai-*.mjs       setup / doctor / smoke / test / clean / stats
src-tauri/src/crawler/       Rust supervisor, installation probe, resource profile
src-tauri/src/research/      SearchProvider adapter (provider id crawl4ai)
src-tauri/src/commands/      crawler_cmds + search_cmds
```

## Process model

1. Rust detects installation (`detect_installation` — venv Python + `import crawl4ai`).
2. On first research request when Ready, the supervisor starts:
   `services/crawl4ai/.venv/bin/python -m coreside_crawler`
3. IPC is **stdio NDJSON** (no HTTP server on the sidecar).
4. Protocol version `"1"`; commands include `health`, `crawl_url`, `crawl_urls`, discovery, `cache_stats`, `cleanup_cache`, `cancel`, `shutdown`.
5. Diagnostics go to stderr; stdout is protocol messages only.

## Rust responsibilities

- Validate public HTTP(S) URLs before handing work to the sidecar
- Own resource profile persistence (`crawler_settings.webResearchResourceProfile`)
- Timeout / cancel / restart policy for the child process
- Map Crawl4AI / discovery output into search result + citation shapes
- Fail closed with `needs_setup` when the engine is not Ready

## Python responsibilities

- Build trusted `BrowserConfig` / `CrawlerRunConfig` (AI cannot supply arbitrary configs)
- Always set `check_robots_txt=True`
- Never enable stealth, undetected browsers, or proxies
- Enforce resource ceilings, domain rate limits, cache quota / LRU cleanup
- Normalize crawl results for citations and media staging

## Discovery vs crawl

Crawl4AI is a **crawler**, not a global search index. Coreside separates:

1. **Discovery** — seeds from user/agent URL or domain (sitemap, RSS, page links, etc.)
2. **Crawl / extract** — fetch allowed URLs and produce Markdown / links / media metadata

Honest empty results are preferred over inventing a search index.

## Pinning

- Package pin: `crawl4ai==0.9.2`
- Preferred Python: 3.11–3.13 in `services/crawl4ai/.venv` only (not global site-packages)
- No background auto-update of Crawl4AI or Chromium

See also: `services/crawl4ai/README.md`.
