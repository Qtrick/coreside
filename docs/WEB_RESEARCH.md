# Web Research

Coreside **Web Research** discovers and inspects web sources for chat citations, page fetch, and media discovery.

Consumer-facing name: **Web Research** / **Web Search**. Internal infrastructure:

| Layer | Role |
| --- | --- |
| **Linkup** | Default indexed discovery and normal public-page retrieval (optional BYOK or hosted secret) |
| **Crawl4AI** | Advanced local crawl, extraction, robots, and browser workflows |
| **Exa** | Optional compatibility discovery provider |

See [EXA_INTEGRATION.md](./EXA_INTEGRATION.md), [SEARCH_OPTIMIZATION.md](./SEARCH_OPTIMIZATION.md), and [CRAWL4AI_INTEGRATION.md](./CRAWL4AI_INTEGRATION.md).

## Behavior

- Free-text queries use Linkup `fast` + `searchResults` when configured. This returns sources only; Coreside chooses follow-ups, synthesis, and citations.
- Direct URLs use Linkup Markdown fetch when Linkup is configured; otherwise they use the SSRF-safe local fetch/crawler path. Crawl4AI remains the advanced path.
- Exa remains an optional compatibility fallback when Linkup is unavailable.
- Without a discovery provider, free-text returns an honest setup notice (no fake SERP).
- Historical Brave citations remain readable; Brave is not the active provider.

## Settings

Development/BYOK: configure Linkup via `LINKUP_API_KEY` in `.env` or the OS-keyring path. Hosted deployments use the Supabase Edge Function's `LINKUP_API_KEY` secret; it is never sent to the desktop. Install Crawl4AI with `npm run crawl4ai:setup` for advanced workflows. See [CLOUD_HOSTING.md](./CLOUD_HOSTING.md).
