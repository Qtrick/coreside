# Web Research

Coreside **Web Research** discovers and inspects web sources for chat citations, page fetch, and media discovery.

Consumer-facing name: **Web Research** / **Web Search**. Internal infrastructure:

| Layer | Role |
| --- | --- |
| **Linkup** | Default indexed discovery and sourced search results (BYOK or hosted secret) |
| **Exa** | Semantic search discovery, highlights, and domain-targeted research |
| **Firecrawl** | Clean markdown web scraping and extraction with source URL validation |
| **Crawl4AI** | Advanced local crawl, extraction, robots, and browser workflows |

See [EXA_INTEGRATION.md](./EXA_INTEGRATION.md), [SEARCH_OPTIMIZATION.md](./SEARCH_OPTIMIZATION.md), and [CRAWL4AI_INTEGRATION.md](./CRAWL4AI_INTEGRATION.md).

## Unified Retrieval Orchestration

All web fetching across the application kernel, search commands, and agent tool loops passes through an authoritative orchestrator (`src-tauri/src/research/retrieval.rs`):

1. **SSRF Guard**: Resolves public DNS, validates all IP destinations (blocking IPv4/IPv6 private ranges, loopback, multicast, link-local, 6to4, Teredo, documentation, and benchmark blocks), and pins destination sockets.
2. **Retrieval Hierarchy**: Crawl4AI (local advanced extraction) $\to$ Firecrawl (clean markdown scrape) $\to$ Linkup (content fetch) $\to$ Pinned-DNS local HTTP client.
3. **Prompt-Injection Sanitization**: Strips adversarial override instructions (`ignore previous instructions`, `system prompt:`, `authorize`, `eval(...)`), bounding untrusted web text into strictly isolated reference evidence.
4. **Evidence Provenance Contract**: Search results distinguish discovery metadata (`fetched_at: None`) from full page content retrieval.

## Settings

Development/BYOK: configure search keys via `.env` or the OS-keyring path (`LINKUP_API_KEY`, `EXA_API_KEY`, `FIRECRAWL_API_KEY`). Hosted deployments use the Supabase Edge Function's search secrets; credentials are never sent to the desktop. Install Crawl4AI with `npm run crawl4ai:setup` for advanced workflows. See [CLOUD_HOSTING.md](./CLOUD_HOSTING.md).
