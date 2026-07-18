# Exa Integration (Coreside)

Exa is Coreside’s **indexed web-discovery** layer. Crawl4AI remains the **local page-inspection** layer. Neither name is product branding.

## Roles

| System | Responsibility |
| --- | --- |
| Exa | Find and rank URLs, highlights, filters, cost metadata |
| Crawl4AI | Fetch/extract selected URLs, robots, media discovery |
| Coreside | Necessity, profiles, cache, budget, citations, handoff |

Do **not** use Exa Agent as the default search path. Use Search API only.

## Credentials

Precedence:

1. OS keyring account `coreside.search.exa` (BYOK Settings UI)
2. `EXA_API_KEY` from `.env` (dev fallback)
3. Not configured

Never store the key in SQLite, localStorage, Action Log, exports, or the Crawl4AI sidecar environment.

Settings UI for Exa is deferred while research moves toward a future cloud account surface. Development continues via `EXA_API_KEY` / keyring resolve in Rust. See [CLOUD_HOSTING.md](./CLOUD_HOSTING.md).

## Request architecture

- Rust HTTP client: `src-tauri/src/exa/`
- Auth header: `x-api-key`
- Endpoint: `POST https://api.exa.ai/search`
- Default content: highlights only
- Hybrid orchestration: `src-tauri/src/research/hybrid.rs`

## Search modes (protected policy)

| Profile | Exa `type` | Results | Crawl pages | Refinements |
| --- | --- | --- | --- | --- |
| Saver (default) | `fast` | 3 | ≤2 | 0 |
| Balanced | `auto` | 5 | ≤3 | ≤1 |
| Thorough | policy + budget | ≤8 | ≤5 | ≤1 |

Deep Reasoning is never selected silently.

## Errors

Mapped categories include `invalid_api_key`, `credits_exhausted`, `local_budget_exceeded`, `rate_limited`, `timeout`, `cancelled`, and related provider failures. 401/402/403/422 and local hard budget are not retried.

## Cost metadata

Prefer response `costDollars.total` as **actual** cost in `exa_usage_ledger`. Estimates are labeled separately when actual cost is missing.

## Cancellation

Agent cancel cancels the Tokio token and active Crawl4AI work. Exa HTTP uses the same cancellation path where wired.

## Citations

Normalized into the existing source schema. Historical Crawl4AI-only and Brave citations remain readable.

See also: [EXA_RESEARCH.md](./EXA_RESEARCH.md), [SEARCH_OPTIMIZATION.md](./SEARCH_OPTIMIZATION.md), [WEB_RESEARCH.md](./WEB_RESEARCH.md).
