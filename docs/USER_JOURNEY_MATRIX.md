# User Journey Matrix (K–P)

**Product:** Coreside v0.1.0  
**Source:** Rows K–P from `docs/MANUAL_ACCEPTANCE_A_AB.md`  
**Run on:** `npm run dev` (Tauri) or packaged build — not Vitest alone.

Mark each: **Pass** / **Fail** / **Skipped** / **Pending** (default: Pending).

| ID | Journey | Steps (summary) | Pass criteria | Status |
| --- | --- | --- | --- | --- |
| **K** | Secondary window | Open a personal tool in a native secondary window; interact in both windows | Window opens; edits sync or conflict banner appears when appropriate | Pending |
| **L** | ZIP package export/import | Export `.coreside-app` ZIP from a tool/app; import with preview + approve | ZIP round-trip succeeds; preview matches content | Pending |
| **M** | Legacy JSON package | Import older JSON v1 package format | Import accepted; migrates or loads without data loss | Pending |
| **N** | EventBus | Create `subscription.create` + `event.dispatch`; restart app | Subscriptions persist; enabled subs still fire after restart | Pending |
| **O** | NDJSON harvest | Dev fixture: raw multi-line ops when schema ops empty | Parser harvests ops; proposal/apply path works | Pending |
| **P** | Recovery Mode | Toggle Recovery in Settings; attempt agent disable | Agent cannot disable recovery; settings toggle works; safe startup path respected | Pending |

## Dependencies per journey

| Journey | Requires |
| --- | --- |
| K | Existing tool/surface, multi-window Tauri route |
| L | Application Kernel package flow (`application_kernel/packages.rs`) |
| M | Legacy import path still compiled |
| N | Runtime V2 events (`runtime_v2/events.rs`) |
| O | Dev fixture or repro transcript with NDJSON ops |
| P | `RecoverySettings` UI + `recovery.rs` enforcement |

## Related automated coverage (not a substitute)

| Journey | Automated partial coverage |
| --- | --- |
| K | `test:navigation-continuity`, multindow docs only |
| L | `test:packages`, `test:application-kernel` |
| M | Package tests in `application_kernel` |
| N | `test:events` |
| O | `response_parser` / fixture tests in Rust |
| P | `test:recovery`, `recovery.rs` unit tests |

## Notes

- Journey **P** is **partial** at the product level: recovery flags persist and agent apply-path is gated, but hiding already-rendered user surfaces in every window is not fully wired (`docs/RECOVERY_MODE.md`).
- No journey in K–P has a recorded passing manual run in this repository as of 2026-07-19.
