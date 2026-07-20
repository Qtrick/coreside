# Hybrid Research Architecture

**Product:** Coreside  
**Assessed:** 2026-07-19

## Flow

1. Hosted Exa discovery via Edge Function `search-gateway` (authenticated, metered).
2. Desktop normalizes bounded results (no Exa secret on device).
3. Source selection in Coreside.
4. Local Crawl4AI sidecar extracts selected pages (Rust-supervised).
5. Bounded excerpts → hosted AI gateway **or** BYOK provider.
6. Citations attached.

## Why Crawl4AI is not in Edge Functions

Crawl4AI needs Python + Chromium/Playwright, high memory, browser lifecycle, and sandboxing. Supabase Edge Functions are unsuitable. Local sidecar remains the extraction plane; a future cloud crawler would be a separate worker (see `docs/CLOUD_CRAWLER_WORKER_ARCHITECTURE.md`).

## Secrets

- Hosted Exa key: Edge Function secret only.
- Crawl4AI never receives Exa or model provider secrets.
