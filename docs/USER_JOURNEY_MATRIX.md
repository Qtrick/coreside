# User Journey Matrix

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Sources:** `docs/MANUAL_ACCEPTANCE_A_AB.md`, package `test:*` / `e2e*` scripts, quick inspection.

Status: `covered` (automated green + recent evidence) · `partial` · `gap` (no recorded pass) · `blocked`.

## Core consumer journeys

| ID | Journey | Automated | Manual / desktop | Gap |
| --- | --- | --- | --- | --- |
| **A** | Cold start | partial (`e2e` scripts exist) | **gap** | No recorded packaged cold-start pass |
| **B** | BYOK connect (key not in SQLite) | partial (`test:ai-access`) | **gap** | Needs real keychain session |
| **C** | Chat stream + cancel | partial (Rust/TS logic) | **gap** | Desktop stream UX unrecorded |
| **D** | Create tool; survives restart | partial (`test:runtime-v2`, surfaces) | **gap** | Restart persistence not signed off |
| **E–G** | Proposal preview / apply / discard | partial (kernel / parser tests) | **gap** | Sticky card UX unrecorded |
| **H** | Inline surface a11y warning | **uncertain** | **gap** | Little automated a11y evidence |
| **I–J** | Queue + revision conflict | partial (`test:queue`, runtime) | **gap** | Conflict banner desktop path |
| **K** | Secondary window | **gap** | **gap** | Multiwindow concurrency unproven |
| **L–M** | ZIP / legacy package import | partial (`test:packages`) | **gap** | Manual round-trip pending |
| **N–O** | EventBus + NDJSON harvest | partial (`test:events`, parser) | **gap** | Restart + fixture UX |
| **P** | Recovery Mode (agent cannot disable) | partial (`test:recovery`) | **gap** | Full surface hide uncertain |
| **Q** | Protected resources | partial (Rust protected checks) | **gap** | Agent attempt scenario |
| **R** | Project isolation | partial (DB isolation queries) | **gap** | Cross-project UI probe |
| **S** | Search budget block | partial (budget code) | **gap** | Needs Exa/Crawl setup |
| **T** | Media MIME reject | partial (media validation) | **gap** | Import UI path |
| **U** | Wallpaper readability | partial (readability tests) | **gap** | Wallpaper **out of scope** this phase |
| **V** | Automations pause/cancel | partial (scheduler tests) | **gap** | Catch-up storm desktop |
| **W–Y** | Palette / theme / narrow layout | partial (`test:layout-overflow`, polish) | **gap** | Full matrix not run |
| **Z** | Restart persistence | partial (migrations) | **gap** | End-to-end restart checklist |
| **AA–AB** | Action Log off + export redaction | partial | **gap** | Export privacy desktop |

## Settings / AI disclosure (consumer-critical)

| ID | Journey | Coverage | Gap |
| --- | --- | --- | --- |
| **S1** | Find AI Access / connect provider without jargon | **gap** | Flat Settings IA; category nav not shipped |
| **S2** | Recovery findable under Advanced (or clear label) | **partial** | Present; not reorganized |
| **S3** | Hosted mode hides provider internals | **blocked** | No hosted adapter |
| **S4** | No contradictory Ready / empty-provider UI | **partial** | Unit coverage; desktop recheck needed |

## Beta blockers (journeys)

1. No complete recorded A–AB (or equivalent) on a packaged build.
2. Settings IA still flat — primary consumer path unclear.
3. Wallpaper visual proof deferred (**out of scope** this docs phase) but residual compositing risks remain for later.
4. Public-beta security evidence incomplete (see `docs/RELEASE_ASSURANCE_RESEARCH.md`).

## Machine-readable

`reports/user-journey-matrix.json`
