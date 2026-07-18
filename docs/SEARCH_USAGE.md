# Search Usage (Coreside)

Local accounting for Exa spend. This is **not** the Exa billing dashboard balance.

## Usage ledger

Table: `exa_usage_ledger`

Records request id, mode, result count, actual vs estimated cost, cache hit, status, optional conversation id. Never stores API keys or full private prompts.

## Actual vs estimated

| Kind | Meaning |
| --- | --- |
| Actual | From Exa `costDollars` when present |
| Estimated | Local fallback when metadata missing — UI must label it |
| Cache hit | No new Exa charge |

Do not present estimates as actual costs.

## Local monthly budget

Optional USD cap in Base Settings → Web Search and Research.

Threshold defaults:

| Threshold | Default | Behavior |
| --- | --- | --- |
| Soft | 75% | Warning; prefer cache; restrict Balanced auto-refinements |
| Critical | 90% | Stronger warning; force Saver-like automatic behavior |
| Hard | 100% | Block automatic Exa; allow cache, direct URL Crawl4AI, and explicit user override paths |

Reset: calendar month (`YYYY-MM` month key).

## Provider credits vs local budget

| Event | Source |
| --- | --- |
| Local budget exceeded | Coreside ledger vs user cap |
| Credits exhausted (HTTP 402) | Exa account |

Coreside cannot top up Exa credits. Cached search and direct Crawl4AI URL inspection remain available when Exa is blocked.

## UI

Settings shows current month spend, request count, credential source, profile, and optional budget. Detailed session lists may use `list_exa_usage`.
