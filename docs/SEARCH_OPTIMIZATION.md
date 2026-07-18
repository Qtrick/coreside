# Search Optimization (Coreside)

Credit-aware policies that keep Exa usage bounded while Crawl4AI does deep inspection only when needed.

## Necessity (before Exa)

Prefer, in order:

1. Local project context
2. Existing citations / chat evidence
3. Cached search results
4. Direct URL → Crawl4AI only (no Exa)
5. Exa search when external discovery is required

Do not call Exa for greetings, pure local state, or when the user already supplied a URL to inspect.

## Profiles

| Profile | Intent |
| --- | --- |
| **Saver** (default) | ≤1 Exa call/turn, 3 results, highlights, 0 refinements, ≤2 crawls |
| **Balanced** | ≤1 Exa call, 5 results, ≤1 refinement if quality insufficient, ≤3 crawls |
| **Thorough** | Up to 2 Exa calls, ≤8 results, deeper mode only with reason + budget |

Only the user changes the active profile in Base Settings. The runtime agent cannot raise limits or disable budgets.

## Query construction

One high-quality query: strip filler, keep entities/dates/intent, avoid full transcripts and private project narrative.

## Deduplication and cache

Normalized fingerprints cover query + mode + filters + result count + content options. Check memory cache → persistent cache → in-flight coalescing before a new Exa request.

## Result-count policy

Default maximum **10**. Profile defaults are lower. Do not expand merely because more results are available.

## Exa → Crawl4AI handoff

1. Rank Exa results
2. Use highlights when sufficient
3. Crawl only top selected URLs within profile caps
4. Merge into the same source records
5. Prefer Crawl4AI over Exa Contents (Contents fallback off by default)

## Media search

Image/video discovery: one bounded Exa search, then Crawl4AI only on selected pages. Imports require Media Library approval.

## Deep modes

Deep / Deep Lite: Thorough + failure of ordinary search + budget check. Deep Reasoning: explicit user approval only.

See [SEARCH_USAGE.md](./SEARCH_USAGE.md) and [EXA_INTEGRATION.md](./EXA_INTEGRATION.md).
