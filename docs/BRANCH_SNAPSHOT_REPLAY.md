# Branch, Snapshot, and Replay (RC3.3 Phase 9)

**Product:** Coreside  
**Phase:** RC3.3 Phase 9 (replay player slice)  
**Last updated:** 2026-08-03  
**Status:** Read-only paced player **Unit Verified** — not true event-stream replay

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

## Honest limitation: transaction stepper ≠ true replay

The backend exposes `list_transactions` (committed application transactions), **not** a chronological redacted provider/form event log.

The player therefore maps each transaction to a **synthetic** event:

> `Committed transaction: {summary}`

This is a **transaction stepper** suitable for paced inspection of commit history. It is **not** Partial Update–parity true replay of stream events, tool calls, or form submissions.

When a redacted event DTO command exists, the player can consume that list without changing the read-only control surface.

## Related surfaces

| Tab | Role |
| --- | --- |
| Branches | Create / open conversation branches |
| Snapshots | Create / view read-only snapshot summaries |
| Replay | Paced read-only transaction stepper (this doc) |
| Inspector | Developer diagnostics (redacted; developer mode) |

## Evidence

- Unit: `npx vitest run src/components/chat/ConversationHistory.test.tsx`
- Report: `reports/replay-results.json` → **Unit Verified** / desktop **Integrated – Not Verified**

## Not claimed

- True chronological provider/form event replay
- Re-apply or time-travel of surface state
- Desktop E2E of the player
- Full Partial Update replay-page parity
