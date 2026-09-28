# Coreside Deep Implementation Pass — 2026-09-28 (catch-up + repair)

**Commit base:** `3c3ef9e5625b3dcaeb14322dc6dfce12325e5fa5`  
**Dirty:** yes (this pass — uncommitted)  
**Public beta:** NOT READY  
**Desktop verified:** no  
**Packaged verified:** no

## Implemented this pass

1. **Durable conversation catch-up cursors**
   - Migration `033_conversation_sync_cursors.sql`
   - Rust `get_conversation_sync_cursor` / `advance_conversation_sync_cursor` (monotonic, conversation-scoped, capped to `MAX(event log sequence)`, existence-checked)
   - Tauri commands + main-window ACL; `get_conversation_events_cmd` ACL registration fixed
   - Frontend `src/lib/sync/conversation-catch-up.ts` — hydrate from SQLite, bounded async pages (200), persist after successful apply, resumable (no 20-page hard stop)
2. **Interactive AI repair turn lifecycle**
   - Exhaustion / provider fail / parse fail → `TurnState::Failed` + `Err` (never TypedTerminal→Published with empty ops)
   - Cancel during repair → `InterruptedRecoverable`
   - Single Error event (no duplicate); queue drain + `take_request` preserved
3. **Progressive preview × interactive repair**
   - `preview_txn.mark_interrupted()` on first probe rejection and on exhaustion
   - Unit tests: interrupted preview cannot accept/paint; interactive_ops_locked gate documented

## Verification (this pass)

| Suite | Result |
| --- | --- |
| Vitest `conversation-catch-up.test.ts` | 15 passed |
| Rust sync_cursor_* / catch_up_pages_* / provider_started_* | 8 passed |
| Rust interactive probe / approval / vendo invariants (filtered) | 14 passed |
| `npm run test:runtime-v2` | 359 passed |
| `npm run test:queue` | 7 passed |
| `npm run test:true-streaming` | 1 passed |
| progressive_* filter | 47 passed |
| `npm run typecheck` | passed |
| Desktop E2E | **not verified** — WebDriver listens on :4445 but `/status` stays `ready:false` (`waiting for webview initialization`) in this agent environment (GUI/webview never becomes ready). Product plugin registration is present under `feature = "e2e"`. |
| Packaged / `release:evidence` full | **not run** this pass |

## Security review (this pass)

- No medium+ findings on sync cursors, repair lifecycle, or rejection information flow
- Approvals already cover single-use CAS / call_hash / tampered input (Vendo-style invariants present and green)
- Quiz rejection still uses `safe_message` + model-safe `fresh_context` (existing tests green)

## Remaining / deferred

- Desktop E2E requires interactive GUI session (environment blocker)
- Selection/caret for contenteditable still partial
- Provider cancel elsewhere still settles as `Failed` while repair cancel uses `InterruptedRecoverable` (intentional improvement; slight inconsistency)
- Full Partial Update parity matrix regeneration for all historical CS-* rows not re-audited end-to-end this pass beyond catch-up/repair/preview

## Intentionally rejected (unchanged)

- Arbitrary HTML/JS/CDN/iframe form POST (Partial Update unsafe model)
