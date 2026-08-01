# Coreside Application Cleansing Audit

**Commit:** `4b5fb1a288b5180860fe1e4fb984defdf296f036`
**Date:** 2026-08-01
**Environment:** macOS 26.5.2 (arm64), Node v24.17.0, npm 11.18.0, rustc 1.96.1, Python 3.14.2

Every number in this document was copied from actual command output on the
machine above. Nothing was carried forward from an earlier report.

---

## 1. Baseline

| Measure | Value |
| --- | --- |
| Frontend source files (`src/**/*.ts,tsx`) | 115 |
| Rust source files (`src-tauri/src/**/*.rs`) | 160 |
| SQLite migrations | 15 (`001` … `015_registered_actions.sql`) |
| Registered Tauri commands | 196 |

### Gate results

| Command | Result |
| --- | --- |
| `npm run typecheck` | Pass |
| `npm run lint` | Pass — 0 errors, 0 warnings |
| `npm test` | 78 passed (14 files) |
| `npm run doctor` | 47/47 checks pass |
| `cargo test` | 301 passed, 0 failed |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --all-targets --all-features` | Compiles; warnings remain, dispositioned in §4 |

### Not executed in this environment

| Gate | Reason | Command a human must run |
| --- | --- | --- |
| Packaged Tauri build | Not attempted this pass; requires a full release toolchain and several minutes of link time | `npm run build` |
| Clean-profile GUI smoke test | No interactive desktop session available to the agent | Launch the packaged app against an empty app-support directory |
| Multi-window approval race | Requires two live windows | See `docs/MANUAL_RELEASE_CHECKLIST.md`, Journey F |
| Windows / Linux compile evidence | macOS-only host | CI matrix `cargo check --target x86_64-pc-windows-msvc` |
| Desktop E2E | No harness exists in the repository | — |

These are recorded as **blocked by environment**, not as passes.

---

## 2. Defects found and fixed

Severity uses the task's P0/P1/P2 scale.

### Trusted core (Rust)

| ID | Severity | Defect | Root cause | Fix |
| --- | --- | --- | --- | --- |
| R1 | P0 | A registered action whose output exceeded the size bound was reported as blocked, but its database writes had already committed | The output-size breaker ran after `handlers::dispatch` returned, outside any transaction | `gateway.rs` now runs the handler and the size check inside a SQLite savepoint and rolls back on either failure. Once-grants burn only after a durable commit. |
| R2 | P0 | `kernel_create_record` / `kernel_query_records` were a second public IPC path to generated data that skipped the audit ledger, breakers, and output bounds | Legacy commands left in place beside the gateway | Both removed from `kernel_cmds.rs` and deregistered in `lib.rs`. No frontend caller existed. `kernel_invoke_registered_action` is now the single choke point. |
| R3 | P1 | Keychain reads ran while the global database mutex was held, stalling every other IPC call behind a blocking OS call | Credential resolution read the DB and the keychain in one function | `credentials/resolve.rs` split into `read_credential_sources` (under lock) and `resolve_from_sources` (after release). Callers in `message_cmds`, `ai_cmds`, and `credential_cmds` updated. |
| R4 | P1 | `get_recent_messages` loaded an entire conversation and truncated in Rust | No `LIMIT` in the query | Query now uses `ORDER BY created_at DESC, rowid DESC LIMIT ?`, then reverses. |
| R5 | P1 | Raw SQLite error text reached the frontend | Blanket `DbError → CommandError` conversion passed the message through | Sqlite errors are logged and replaced with a generic `storage_error`; `revision_conflict` maps to a distinct `conflict` code. |
| R6 | P1 | Generated-data migration could panic on a record missing `_id` | `unwrap()` in the `add_field` backfill | Records without `_id` are skipped. |
| R7 | P1 | Concurrent writers failed immediately instead of waiting | No `busy_timeout` pragma | `PRAGMA busy_timeout = 5000` and `synchronous = NORMAL` added at connection setup. |
| R8 | P1 | A daily or weekly automation scheduled inside a DST gap could compute no next run and silently stop | `compute_next_after` returned `None` for non-existent local times | `resolve_local` handles gaps and ambiguity; the scheduler disables an automation whose next run cannot be computed so the user can see it. |
| R9 | P1 | Once the event-bus loop breaker tripped, the surface stayed suspended for the entire process lifetime | `suspended: bool` was set on loop detection and only `unsuspend` cleared it — and `unsuspend` had no caller | Replaced with `suspended_until: Option<Instant>`. The breaker stops a runaway loop, so it releases after `EVENT_SUSPENSION_SECS` once the surface goes quiet. `unsuspend` still clears it early. |
| R10 | P1 | The event bus retained every idempotency key it had ever seen, for the life of the process | `seen_idempotency: HashSet<String>` was never pruned | Bounded at `MAX_IDEMPOTENCY_KEYS` with FIFO eviction. |
| R11 | P1 | A rate-limited or rejected event burned its idempotency key, so the legitimate retry was rejected as a duplicate | The key was claimed before the remaining dispatch checks ran | `dispatch` releases the key when `dispatch_checked` returns an error. |
| R12 | P1 | The documented "three crashes suspends the application" recovery path could never trigger | `manifest::record_crash` had no caller, so `crash_count` was always 0 and the `crash_count >= 3` clause in `enter_safe_startup` never matched | `record_build_failure` now records a crash when the failure is non-retryable. Retryable failures are still display-only. |

### Frontend

| ID | Severity | Defect | Root cause | Fix |
| --- | --- | --- | --- | --- |
| F1 | P0 | A reply that arrived after the user switched chats wrote Conversation A's messages into Conversation B's view | `sendMessage` and `navigateToChat` did not re-check the active conversation after their awaits | Both re-check `activeConversationId` before every conversation-scoped `set`. |
| F2 | P0 | Registered-action failures inside a generated tool were only `console.warn`ed | No user-visible error channel in the renderer | `ToolRenderer` shows a dismissible `role="alert"` banner for action and persistence failures. |
| F3 | P1 | The settings-change branch of `sendMessage` awaited `api.getSettings()` *after* the staleness guard and then wrote conversation state | Second await reopened the race that F1 closed | Re-checks after the await; the global appearance patch still applies, the conversation-scoped fields do not. |
| F4 | P1 | Tool Canvas header actions (undo, open in window, restore) rejected silently | Unhandled promise rejections | `runHeaderAction` surfaces failures in a canvas-level error banner. |
| F5 | P1 | `selectTool` had the same stale-result race as chat navigation | Missing active-ID guard | Guard added. |
| F6 | P1 | `InlineSurfaceCard`'s hydration effect captured a stale `surface` | Stale closure, flagged by `react-hooks/exhaustive-deps` | `latestSurfaceRef` tracks the current surface. |
| F7 | P1 | Dialogs declared `aria-modal="true"` but had no focus trap and did not restore focus | Each dialog rolled its own markup | `ModalPortal` now owns the shared contract: focus moves in, Tab cycles, Escape closes the topmost dialog, focus returns on close. |
| F8 | P1 | Closing a non-topmost dialog pulled focus out of the dialog still on screen | Unmount always restored the pre-modal element | Restore now respects the modal stack and moves focus into the remaining top dialog instead. |
| F9 | P1 | Optimistic tool-state writes swallowed persistence failures | `setValueOptimistic` ignored the rejection | The failure is reported through the same banner as F2. |
| F10 | P2 | Inline surfaces shipped a permanently disabled "Archive" button | Placeholder control for unimplemented behaviour | Removed. Archive can return when the behaviour exists. |
| F11 | P2 | "PNG Snapshot" export fabricated an image rather than exporting the tool | Placeholder implementation | Removed from `ExportDialog` and from the backend's format list. |
| F12 | P2 | Pending approvals woke every window on a 4-second timer | Polling stood in for backend events | The backend emits `runtime-approvals-changed` from invoke, decide, revoke, and the scheduler. Focus refresh is retained as a cheap fallback. |
| F13 | P2 | `Sidebar`, `SettingsPanel`, and `ToolCanvas` subscribed to the whole store, so they re-rendered on every streaming token | `useAppStore()` with no selector | Converted to `useShallow` selector objects. |

---

## 3. Subsystem disposition

| Subsystem | Status | Evidence |
| --- | --- | --- |
| Registered-action gateway | `FIXED` | R1, R2; savepoint rollback regression tests; doctor `gateway.single_choke` |
| Approvals and grants | `PARTIAL` | CAS and once-consumption unit-tested; multi-window desktop race `MANUAL_VERIFICATION_REQUIRED` |
| Audit ledger | `VERIFIED_HEALTHY` | Events persist through the gateway; note `list_action_events_for_request` is unused (§4) |
| Recovery Mode | `PARTIAL` | R12 wired the crash signal; no UI reporter yet — see `docs/RECOVERY_MODE.md` |
| Automations / scheduler | `FIXED` | R8; orphaned automations are now disabled rather than silently stalled |
| Event bus | `FIXED` | R9, R10, R11 with regression tests |
| Credentials | `FIXED` | R3; keys remain in OS storage, never in SQLite or logs |
| Database and queries | `FIXED` | R4, R5, R6, R7 |
| Migrations | `PARTIAL` | 15 migrations run; fixture corpus still thin — `docs/MIGRATION_ASSURANCE.md` |
| Chat and streaming | `FIXED` | F1, F3 |
| Tool renderer / inline surfaces | `FIXED` | F2, F6, F9, F10; doctor `ui.inline_pending_approval` |
| Zustand state | `OPTIMIZED` | F13 |
| Modals / accessibility | `FIXED` | F7, F8 |
| Export / import | `FIXED` | F11; doctor `packages.authority_stripped` |
| Doctor / CI | `VERIFIED_HEALTHY` | 47/47; `.github/workflows/ci.yml` present |
| Media, wallpapers, search, crawler | `DEFERRED` | Not modified this pass; no defect found in the areas inspected, but not exhaustively audited |
| Packaging, cross-platform, E2E, performance | `BLOCKED` | See §1 |

---

## 4. Rust dead code: why it was kept

`cargo clippy` reports a large set of unused items. They were traced
individually rather than deleted or silenced. Confirmed *duplicates* of wired
code were removed. The remainder are **features whose logic exists but whose
production call path does not reach them** — deleting them would erase the
evidence that the feature is incomplete.

The full table lives in `docs/KNOWN_ISSUES.md` under "Rust dead code". The
highest-impact entries are:

1. **Preservation** — the `component_preservation` table and its Rust API are
   never used; preservation is implemented frontend-only.
2. **Runtime ceilings** — `MAX_SNAPSHOTS_PER_CONVERSATION` and
   `MAX_BRANCH_DEPTH` are documented but not enforced, so snapshot and branch
   growth are unbounded.
3. **Dependency-aware delete** — `application_dependencies` is never written, so
   deletion impact warnings can never fire.
4. **Surface deletion** — the `surface.delete` operation is accepted and
   recorded for replay but deletes nothing.

None is reachable by a user today, so none is a live defect. Each is a real gap
in feature completeness and should be scheduled deliberately.

---

## 5. Remaining risks

| Risk | Severity | Note |
| --- | --- | --- |
| Multi-window approval exactly-once not observed on a real desktop build | P1 evidence | Logic is CAS-protected and unit-tested; the desktop run has not happened |
| No packaged build this pass | P1 evidence | `npm run build` not executed |
| No desktop E2E harness | P1 evidence | Nothing committed |
| No performance baseline | P2 | `docs/PERFORMANCE_BASELINE.md` does not exist |
| Migration fixture corpus thin | P2 | Fresh-database path is covered; realistic upgrade fixtures are not |
| Windows / Linux unverified | P2 | Do not claim support |
| `surface.delete` silently succeeds without deleting | P2 | Accepted operation with no effect |
| Snapshot and branch growth unbounded | P2 | Limits defined but unenforced |
