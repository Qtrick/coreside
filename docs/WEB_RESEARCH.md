# Web Research

Coreside **Web Research** discovers and inspects web sources for chat citations, page fetch, and media discovery.

Consumer-facing name: **Web Research** / **Web Search**. Internal infrastructure:

| Layer | Role |
| --- | --- |
| **Exa** | Indexed open-web discovery and ranking (optional API key) |
| **Crawl4AI** | Local crawl, extraction, robots, media discovery |

See [EXA_INTEGRATION.md](./EXA_INTEGRATION.md), [SEARCH_OPTIMIZATION.md](./SEARCH_OPTIMIZATION.md), and [CRAWL4AI_INTEGRATION.md](./CRAWL4AI_INTEGRATION.md).

## Behavior

- Free-text queries use Exa when configured (Saver profile by default).
- Direct URLs skip Exa and crawl with Crawl4AI only.
- Without Exa, free-text returns an honest setup notice (no fake SERP).
- Historical Brave citations remain readable; Brave is not the active provider.

## Settings

Web Research consumer Settings UI is currently hidden (cloud-oriented later). Development: configure Exa via `EXA_API_KEY` in `.env`; install Crawl4AI with `npm run crawl4ai:setup`. See [CLOUD_HOSTING.md](./CLOUD_HOSTING.md).
