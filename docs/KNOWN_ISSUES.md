# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-01

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## Assurance infrastructure

| Issue | Severity | Status |
| --- | --- | --- |
| Full E2E / WebDriver not configured | High (for beta) | **Open** — no `tauri-driver`, no green E2E suite |
| Manual A–AB / journeys K–P not recorded | High (for beta) | **Open** — see `docs/MANUAL_RELEASE_CHECKLIST.md` |
| No CI workflows | Medium | **Fixed** — `.github/workflows/ci.yml` + `npm run doctor` |
| Migration fixture testing | Medium | **Fixed** — `npm run test:migrations` covers fresh + upgrades from 006/011/012/013/014/015 with FK checks |
| Desktop E2E foundation | High (for beta) | **Partial** — foundation doc at `docs/E2E_FOUNDATION.md`; harness not implemented |
| Packaged-build clean-profile smoke | High (for beta) | **Open** — not executed this cycle |
| Performance baselines | Medium | **Open** — not measured |

## Generated application runtime (2026-08-01)

| Issue | Severity | Status |
| --- | --- | --- |
| Legacy `kernel_*_record` IPC beside gateway | P2 | **Fixed** — `kernel_create_record` / `kernel_query_records` removed; `kernel_invoke_registered_action` is the only public data path |
| Handler output size checked after write | P1 | **Fixed** — the handler runs inside a SQLite savepoint and rolls back when the output bound trips |
| Agent chat path inventing registered-action calls | P2 | **Deferred** — UI/automation invoke only |
| Multi-window approval race manual QA | P1 evidence | **Open** — unit CAS covered; desktop not re-shot |
| InlineSurface registered-action wiring | P2 | **Fixed** — inline surfaces call the same gateway as Tool Canvas (`doctor: ui.inline_pending_approval`) |
| Approval refresh woke every window on a 4s timer | P2 | **Fixed** — backend emits `runtime-approvals-changed`; focus refresh retained as fallback |
| Event-bus loop suspension was permanent for the process | P1 | **Fixed** — suspension expires after `EVENT_SUSPENSION_SECS`; `unsuspend` clears it early |
| Event-bus idempotency set grew without bound | P1 | **Fixed** — capped at `MAX_IDEMPOTENCY_KEYS`, and a failed dispatch releases its key |
| `record_crash` had no caller, so `crash_count` never left 0 | P1 | **Fixed** — a non-retryable build failure now records a crash, enabling the documented three-strike suspend |

## Automated test status (2026-08-01, commit 4b5fb1a)

Counts below are copied from actual command output on macOS 26.5.2 (arm64),
Node v24.17.0, rustc 1.96.1. Re-run before quoting them anywhere else.

| Suite | Result |
| --- | --- |
| `npm run typecheck` | Pass |
| `npm run lint` | Pass — **0 errors, 0 warnings** |
| `npm test` | **78/78 pass** (14 files) |
| `npm run doctor` | **47/47 checks pass** |
| `cargo test` | **301/301 pass** |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --all-targets --all-features` | Passes with warnings — see "Rust dead code" below |

## AI access

| Issue | Severity | Status |
| --- | --- | --- |
| Hosted adapter (`coreside_hosted`) not built | Expected | **Open** — `hosted_connected: false`; never fabricate “Coreside AI connected” |
| Env credentials brand as “Coreside AI” | P1 | **Fixed** — consumer label is **AI Access / Ready**; “Coreside AI” reserved for real hosted |
| Env credentials leaked OpenRouter / model slug | P1 | **Fixed** — disclosure policy + Settings + catalog gating |
| Message bubble model attribution under env mode | P2 | **Fixed** — gated on `showModelIdentity` |
| Real Tauri screenshot retest of Settings card | P1 evidence | **Open** — unit coverage only; desktop not re-shot this cycle |

## Generated tools

| Issue | Severity | Status |
| --- | --- | --- |
| Stub tools render as “Text” / “Button” | P1 | **Mitigated** — prompt examples + pack validation reject missing/placeholder labels; empty creates rejected. Existing stub tools already saved are unchanged until user asks to rebuild. |
| Model still under-builds complex tools | P2 | **Open** — validation blocks worst stubs; quality still depends on the model |

## Product partial implementations

| Issue | Severity | Status |
| --- | --- | --- |
| Recovery Mode incomplete | Medium | **Partial** — safe startup, execution gating, and last-known-good restore work and are covered by tests; the crash signal is now wired (non-retryable build failure), but no UI reports a render failure yet, so three-strike suspend is only reachable through the IPC command |
| Search requires Exa + Crawl4AI | Low | By design when keys missing |
| Windows / Linux release verification | High (if claimed) | **Open** — macOS primary |

## Lint

| Issue | Severity | Status |
| --- | --- | --- |
| 5 ESLint react-refresh / hooks warnings | Low | **Fixed** — the hooks warning was a real stale closure in `InlineSurface`; the four react-refresh warnings were resolved by moving non-component exports into `project-icon.ts`, `use-context-menu.ts`, `proposal-metadata.ts`, and `search-metadata.ts` |

## Rust dead code

`cargo clippy` reports unused items that are **not** merely stylistic: they are
features whose logic exists but whose production call path does not reach them.
They are tracked here rather than deleted, because deleting them would hide the
gap. None of them are reachable by a user today, so none is a live defect.

| Area | Unused symbols | What is actually missing |
| --- | --- | --- |
| Preservation | `PreservationPolicy`, `should_preserve`, `upsert_preservation`, `get_preservation`, `list_preservation_for_surface` | The `component_preservation` table and Rust API exist, but state preservation is implemented frontend-only in `src/lib/preservation.ts`. Rust never participates. |
| Dependency-aware delete | `DependencyEdge`, `add_dependency`, `dependency_impact`, `deletion_warnings` | `application_dependencies` is never written, so pre-delete impact warnings can never fire. |
| Surface deletion | `remove_subscriptions_for_surface` | The `surface.delete` operation is accepted and recorded but performs no deletion, so there is nothing to clean up yet. |
| Runtime ceilings | `MAX_INLINE_SURFACES_VISIBLE`, `MAX_SNAPSHOTS_PER_CONVERSATION`, `MAX_BRANCH_DEPTH`, `MAX_REPLAY_OPS_LOADED`, `MAX_CANVAS_OBJECTS`, `MAX_CODE_EDITOR_CHARS` | Documented in `docs/PERFORMANCE_LIMITS.md` but never enforced. Snapshot and branch growth are currently unbounded. |
| Application jobs | `update_job_progress` | `kernel_create_job` / `kernel_get_job` are registered but absent from `src/lib/tauri.ts`, so no UI tracks job progress. |
| Chat summaries | `set_chat_summary` | Per-chat summaries are read if present but never written. |
| Provider conformance | `select_application_profile`, `seed_provider_profiles` | Profiles are stored but not consulted when applying agent operations. |
| Crawler cancel | `crawler::cancel_request` | An in-flight sidecar crawl cannot be cancelled. |
| Wallpaper presets | `trusted_presets`, `preset_by_name` | Duplicate of the authoritative `CANVAS_PRESETS` in `src/types/wallpaper.ts`. |
| Draft cleanup | `delete_draft` | Drafts can be saved and read but never deleted. |

## Not issues (explicit non-goals this phase)

- Hosted AI backend / billing / accounts
- Enterprise SSO / org admin
- Marketplace / arbitrary package install

## Reporting

New findings: add to `reports/release-findings.json` with severity P0/P1/P2.
