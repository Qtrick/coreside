# Branch, Snapshot, and Replay (RC3.3 Phase 9 → RC3.4 timeline slice)

**Product:** Coreside  
**Phase:** RC3.3 Phase 9 + RC3.4 immutable timeline vertical slice  
**Last updated:** 2026-08-04  
**Status:** Read-only paced player **Unit Verified** — prefers redacted timeline when present; otherwise transaction stepper. **Not** re-execution.

## What landed

History → **Replay** hosts a minimal **ReplayPlayer**:

| Control | Behavior |
| --- | --- |
| Previous / Next | Step a local cursor over redacted event summaries |
| Play / Pause | Auto-advance with `setTimeout` (`REPLAY_STEP_MS` / speed) |
| Speed | Toggle 1x / 2x |
| Return to live | Clear cursor; exit inspection |

Display shows the **current event summary only**.

## HARD RULE

Replay **never**:

- Reinvokes a provider
- Re-applies transactions (`applyOperations` / `apply_transaction`)
- Resubmits forms
- Mutates surfaces, queue, or conversation state

Enforced in UI copy and code comments on `ReplayPlayer` / `ReplayEvent`.

## Event sources (preference order)

1. **Redacted turn timeline** (`list_turn_timeline_cmd` / table `turn_timeline_events`) when any events exist for the conversation.
2. **Fallback:** `list_transactions` mapped to synthetic `Committed transaction: {summary}` events.

Timeline kinds (schema): `user_request`, `provider_start`, `text_checkpoint`, `operation_received`, `operation_accepted`, `operation_rejected`, `preview_update`, `approval`, `commit`, `failure`, `cancellation`, `completion`.

**This slice emits best-effort** (never blocks the turn): `commit`, `failure`, `cancellation`, `completion` on the `send_message` path. Other kinds are reserved for later append points.

Payloads are redacted summaries only — never secrets, prompts, or raw provider bodies.

## Honest limitation

Even with timeline rows, Replay is **inspection of stored redacted events**, not Partial Update–parity true replay of stream tokens, tool calls, or form submissions. Timeline presence does **not** mean re-execution.

## Related surfaces

| Tab | Role |
| --- | --- |
| Branches | Create / open conversation branches |
| Snapshots | Create / view read-only snapshot summaries |
| Replay | Paced read-only timeline or transaction stepper (this doc) |
| Inspector | Developer diagnostics (redacted; developer mode) |

## Evidence

- Unit: `npx vitest run src/components/chat/ConversationHistory.test.tsx`
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib runtime_v2::turn_timeline`
- Report: `reports/replay-results.json` → **Unit Verified** / desktop **Integrated – Not Verified**

## Not claimed

- Full chronological coverage of every kind on every turn
- Re-apply or time-travel of surface state
- Desktop E2E of the player
- Full Partial Update replay-page parity
