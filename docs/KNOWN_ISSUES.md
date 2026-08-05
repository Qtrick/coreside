# Known Issues

**Product:** Coreside v0.1.0  
**Last updated:** 2026-08-04  
**Evidence:** regenerate with `npm run release:evidence` — do not treat July counts as current.

## RC3.6 notes (2026-08-04)

| Issue | Severity | Status |
| --- | --- | --- |
| Journey 11 tool-window ACL denial | P1 evidence | **Isolated Desktop Verified** — 15 sensitive invokes + cross-tool write denied on HEAD (`command-authority-results.json`); full orchestrator **passed_partial** (J7) — not a suite Desktop Verified pass |
| Multimodal image + tool-result send | P1 | **Unit Verified** — OpenAI/Ollama/Anthropic/Gemini family adapters + `validate_provider_send`; desktop multimodal chat **not_run** (see [ATTACHMENT_LIFECYCLE.md](./ATTACHMENT_LIFECYCLE.md)) |
| Local AI privacy routing | P1 | **Partial** — `access_mode.rs` + Composer disclosure; Journey 17 **not_run** |
| Hosted Coreside AI gateway + billing | P2 hosted | **Scaffolded** — Edge Functions + entitlements SQL unit-tested; private alpha **Not ready**; separate track |
| Preview overlay cross-chat leak | P1 | **Mitigated** — `clearPreviewOverlaysMatching` requires `conversationId` for tool/surface clears; unscoped Sync does not wipe unrelated overlays (14 vitest) |
| Sync / Conflict residual global bus | P1 | **Partial** — scoped `subscribe_conversation_sync` primary; residual `agent-turn` only when no subscriber; multi-window skip desktop **not_run** |
| Narrow assurance (RC3.6) | P2 evidence | **Passed (narrow)** — `npm run assurance:report` 0 findings on HEAD (dirty tree); not a full release pass |
| Full desktop E2E 1–16 + packaged smoke | P1 | **Partial** — `e2e-results.json` **passed_partial** (J7) on fp `3c3fa498…`; J1–16 passed; J17–18 **not_run**; not a suite Desktop Verified pass; packaged smoke **not_run** |

## RC3.4 notes

| Issue | Severity | Status |
| --- | --- | --- |
| Onboarding desktop E2E (welcome + tour) | P2 evidence | **Passed (journey)** — J15–16 passed in orchestrator on fp `3c3fa498…`; full suite **passed_partial** (J7) — not a suite Desktop Verified pass |
| Contextual tips / What’s new | P3 | **Unit landed** — vitest eligibility; desktop **not_run** |
| Help category remount when Settings already open | P3 | Mitigated via `coreside:settings-category` event; still verify manually |
| Tour sample seed/cleanup IPC unused in UI | P3 | Commands ACL-registered; no Help button yet; cleanup is id-scoped |
| Baseline report migrations/command counts | P2 evidence | Counts corrected to migrations=17 / commands=220; re-run `npm run audit:current-source` after commit for fingerprint parity |
| Preview overlay conversation scoping | P1 | **Mitigated** — overlays keyed with conversationId; Sync/Conflict clear even when reload skipped; interrupt/error/finalize clear per-conversation |
| Consumer language audit | P3 evidence | Heuristic `npm run audit:consumer-language` — review findings; not Desktop Verified |

Tracked gaps affecting release assurance. Not an exhaustive bug list.

## RC3.2 public-beta blockers (current)

| Issue | Severity | Status |
| --- | --- | --- |
| Tauri `commands.deny` is global (blocks main) | P0 | **Fixed** — removed `coreside-tool-deny-sensitive` from `tool-window` capability; isolation = allowlist `coreside-tool-scoped` only (Tauri 2.11 `resolve_access` ignores window on deny) |
| Wallpaper atomic persistence + slider coalesce | P1 | **Partial** — desktop pixel sampling **passed** (Journey 13, fp `3c3fa498…`); full suite **passed_partial** (J7); packaged pixels **not_run** |
| Global `agent-turn` text eavesdrop | P1 | **Mitigated** — Channel for interactive send; Journey 14 **passed** on fp `3c3fa498…`; full suite **passed_partial** (J7) |
| Journey 12/13 desktop execution | P1 evidence | **Desktop Verified (journey)** — J12/J13 passed on fp `3c3fa498…`; full orchestrator **passed_partial** (J7) — not a suite Desktop Verified pass |
| Full turn registry / delta-only IPC | P1 | **Partial** — frontend `turnsById` + delta apply landed; reconnect / delta-only wire / concurrent UI still open |
| Typed StructuredUserInput (not text delimiter) | P1 | **Unit Verified** — typed seal + trust gate; delimiter not authority; desktop E2E **not_run** (see [STRUCTURED_FORMS_AND_CONTEXT.md](./STRUCTURED_FORMS_AND_CONTEXT.md)) |
| Progressive preview transaction | P1 | **Partial** — live NDJSON preview + speculative surface paint landed; JSON-blob progressive / desktop E2E remain open (see [PROGRESSIVE_PREVIEW_TRANSACTION.md](./PROGRESSIVE_PREVIEW_TRANSACTION.md)) |
| Attachment crash suite + scoped ACL | P1 | **Partial** — crash/reconcile + authorize **Unit Verified**; Journey 11 ACL **Isolated Desktop Verified**; full suite **passed_partial** (J7) |
| Coherent profile restore | P1 | **Partial** — journal stage ladder + mid-swap → Recovery **Unit Verified** (`reports/restore-transaction-results.json`); packaged FS swap proof **open** (see [DURABLE_PROFILE_ARCHITECTURE.md](./DURABLE_PROFILE_ARCHITECTURE.md)) |
| Global queue metadata emit | P1 | **Closed** — conversation-scoped `subscribe_conversation_queue` Channel; no process-wide `agent-queue-changed` (see [QUEUE_COORDINATION.md](./QUEUE_COORDINATION.md); Unit Verified, desktop **not_run**) |
| Sync / Conflict scope match | P1 | **Partial** — Sync carries `conversationId` + `surfaceIds`/`toolIds`; frontend `shouldApplyAgentTurnSync` / `shouldShowAppConflict` skip cross-conversation reload/banner; desktop multi-window **not_run** |
| Multimodal Image parts | P1 | **Unit Verified (RC3.6)** — native parts all families + send-path validation; desktop **not_run** (see [ATTACHMENT_LIFECYCLE.md](./ATTACHMENT_LIFECYCLE.md)) |
| Replay player vs true event replay | P2 | **Partial** — paced read-only `ReplayPlayer` steps synthetic “Committed transaction” events from `list_transactions`; **not** true provider/form event-stream replay (see [BRANCH_SNAPSHOT_REPLAY.md](./BRANCH_SNAPSHOT_REPLAY.md)) |

## RC3.1 public-beta blockers (carry-forward)

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E Journey 12 (true streaming) | P1 evidence | **Spec written; not_run** |
| Full desktop E2E suite + packaged smoke | P1 | **Open** |
| Anthropic / Gemini live SSE | P1 product | **Unit Verified** — live `chat_stream` SSE adapters; desktop against real keys **not_run** |

## Assurance infrastructure

| Issue | Severity | Status |
| --- | --- | --- |
| Desktop E2E harness | High (for beta) | **Partial** — `e2e-results` **passed_partial** (J7) on fp `3c3fa498…`; J1–16 passed; J17–18 **not_run**; not a suite Desktop Verified pass |
| Packaged Tauri build | High (for beta) | **Partial** — clean-profile packaged smoke still needs recorded evidence |
| Misleading evidence script names | Medium | **Fixed** — renamed overclaiming `parity`/`replay`/`inspector` scripts |

## Generated application runtime (2026-08-01)

| Issue | Severity | Status |
| --- | --- | --- |
| `surface.delete` was a no-op | P0 | **Fixed** — real delete/archive/restore + draft/continuity cleanup |
| Dictation “coming soon” generatable | P1 | **Fixed** — pack removed; compatibility fallback only |
| Draft cleanup on conversation delete | P1 | **Fixed** |
| Crawler per-request cancel unused | P2 | **Fixed** — `cancel_active` uses shared cancel protocol; agent Stop cancels crawls |
| Production tauri bridge included mocks | P1 | **Fixed** — `src/lib/tauri/` split; mocks async-only outside Tauri |
| Multi-window approval race desktop | P1 evidence | **Partial** — Journey 7 asserts secondary has no duplicate approval UI; concurrent cross-window approve race not exercised (WebDriver single-window) |
| InlineSurface registered-action wiring | P2 | **Fixed** |
| Event-bus loop suspension permanent | P1 | **Fixed** |
| Event-bus idempotency unbounded | P1 | **Fixed** |
| `record_crash` unused | P1 | **Fixed** — three-strike suspend via build failures |
| Create-side dependency edges unwired | P2 | **Open** — delete cleans edges; create-side still TODO |

## Automated test status

Re-run before quoting. Prefer `reports/release-evidence.json`.

| Suite | Command |
| --- | --- |
| Typecheck / lint / vitest | `npm run typecheck` / `lint` / `test` |
| Rust / migrations | `npm run test:rust` / `test:migrations` |
| Doctor | `npm run doctor` |
| Desktop E2E | `npm run e2e:desktop` |
| Packaged build | `npm run build` + `package:scan` |
