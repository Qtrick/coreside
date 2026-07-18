# Exa Research Notes (Coreside)

**Date:** 2026-07-17  
**Sources:** [Search API](https://docs.exa.ai/reference/search), [Pricing](https://exa.ai/pricing?tab=api), [Changelog](https://exa.ai/docs/changelog), [x402 guide](https://exa.ai/docs/reference/x402-guide)

## Endpoint

- Base: `https://api.exa.ai`
- Search: `POST /search`
- Auth: header `x-api-key: <key>` (or Bearer)
- API version documented as OpenAPI 2.0.0 surface

## Credential precedence (release-blocking)

1. Active OS keyring / credential-manager secret for Exa  
2. Developer `.env` fallback (`EXA_API_KEY`) — never shown in UI; Settings may say “Using development environment credential”  
3. Not configured → setup / local-only Crawl4AI path (no fake SERP)

Key stays in trusted Rust core only. Do not pass Exa credentials into the Crawl4AI sidecar or the webview.

## Search `type` values (current)

| Type | Use in Coreside |
| --- | --- |
| `fast` / `instant` | Saver / navigational / media discovery |
| `auto` | Balanced default |
| `deep-lite` / `deep` | Thorough only, budget-gated |
| `deep-reasoning` | Never automatic; explicit approval only |

**Do not use Exa Agent as default search.** Agent API is out of scope for the hybrid phase; do not wire it as an implicit fallback when Search fails.

## Contents policy

Prefer `contents: { highlights: true }` on search. Avoid full `text` / `summary` / subpages by default.  
Crawl4AI handles deeper page extraction for selected URLs. Exa Contents fallback: **off** by default.

## Crawl4AI handoff bounds (release-blocking)

After Exa returns results, Coreside ranks/selects then hands off **only** the top URLs to Crawl4AI:

| Profile | Max Exa results | Max Crawl4AI pages | Refinements |
| --- | --- | --- | --- |
| Saver | 3 | ≤2 | 0 |
| Balanced | 5 | ≤3 | 1 |
| Thorough | ≤8 | ≤5 | policy + budget |

Hard stops: local budget exhausted, user cancel, compliance/SSRF reject, or engine not Ready. No unbounded crawl fan-out from Exa result lists.

## Pricing (verified March 2026 update)

| Mode | Approx. | Notes |
| --- | --- | --- |
| Search (`instant`/`auto`/`fast`) | **$0.007 / request** (up to 10 results) | Highlights/text for first 10 included in current bundled pricing |
| Deep / Deep-lite | **$0.012 / request** | |
| Deep-reasoning | **$0.015 / request** | |
| Extra results >10 | **$0.001 / result** (per 1k pricing tables) | Coreside hard-caps ≤10 |
| Contents per type | **$0.001 / page** | Prefer Crawl4AI instead |
| Summaries | **$0.001 / page** | Avoid by default |

Response includes `costDollars.total` (and nested breakdown) — persist as **actual** cost.

## Budget vs Exa balance (release-blocking)

| Concept | Owner | Meaning |
| --- | --- | --- |
| Local monthly budget | Coreside settings + usage ledger | Optional user spend cap; blocks further Exa calls when reached |
| Exa account balance / credits | Exa billing | Provider-side; surfaces as HTTP **402** / `credits_exhausted` |

These are **not** the same value. UI and errors must distinguish “local budget reached” from “Exa credits exhausted.” Local budget does not query or mirror Exa balance.

## Rate limits

Public guidance ~**10 QPS** default; respect `429` + `Retry-After`. Max **2** transient retries with jitter. Never retry 401/402/403/422/local budget.

## Errors to map

401 → invalid_api_key · 402 → credits_exhausted · 403 → insufficient_permission · 422 → invalid_request · 429 → rate_limited · 5xx → provider_unavailable · timeout/cancel as usual.

## SDK decision

**Direct HTTP from Rust** (`reqwest`) — key stays in trusted core; no Exa key in Python sidecar or React. Official `exa-js` / `exa-py` not used in production path.

## Coreside defaults

- Profile default: **Saver** (`fast`, 3 results, highlights, ≤2 Crawl4AI pages, 0 refinements)
- Balanced: `auto`, 5 results, ≤3 crawls, 1 refinement
- Thorough: up to 8 results, ≤5 crawls, deep only when policy + budget allow
- Exa Contents fallback: off
- Monthly local budget: optional user setting (**does not equal** Exa account balance)
