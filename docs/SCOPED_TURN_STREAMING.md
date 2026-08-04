# Scoped Turn Streaming (RC3.2 Phase 3 → RC3.4 polish)

**Product:** Coreside  
**Status:** Interactive Channel + Sync/Conflict conversation-scoped subscriber fan-out + minimal turn registry  
**Access date:** 2026-08-04

## Problem

`app.emit("agent-turn", …)` is process-wide. Any webview with `core:event:default` (including tool windows) can receive another conversation’s assistant text or private action/error/operation payloads. Client-side filtering is defense in depth, not authorization.

## Design (this slice)

Interactive `send_message` takes a required Tauri `Channel<AgentTurnEvent>` (`onEvent` from the frontend).

| Kind | Channel present (interactive send) | Channel absent (queue drain) |
| --- | --- | --- |
| `text` | Channel only — **never** global emit | Skipped (privacy) |
| `action` / `error` / `operation` / `previewSurface` | Channel only — **never** global emit | **Dropped** (log at debug); no global leak |
| `sync` / `conflict` | Prefer `subscribe_conversation_sync` fan-out; if any subscriber delivered, **skip** invoke Channel (no double apply). Else invoke Channel if present. Global `app.emit("agent-turn")` **only** when neither scoped path succeeded | Same ordering; global only if no successful scoped send |

Frontend:

- `api.sendMessage` creates `@tauri-apps/api/core` `Channel` and passes `onEvent`.
- `app-store.sendMessage` applies Channel text/action/error/operation/preview **and** Sync/Conflict for that send.
- `app-store` also calls `subscribeConversationSync` when the active conversation is set (and tool windows subscribe when `surf-{toolId}` resolves a `conversationId`).
- `listenAgentTurn` remains as defense-in-depth for **residual** global Sync/Conflict only; private kinds are ignored.
- Tool-window bootstrap already ignores text/action/error/operation — keep that (eavesdropping denial).

### Sync subscriber ACL

- `subscribe_conversation_sync` on `coreside-main-default` and `coreside-tool-scoped` (tool windows may subscribe when they know the conversation).
- Multi-window: sync subscribers are keyed by **window label** (replace on remount for the same window; distinct labels fan out so main + tool both receive).

### Text payload shape (delta-primary)

`AgentTurnEvent::Text` may include optional `turnId`, `sequence`, and `delta`.

- Ordinary events are **delta-primary**: cumulative `text` is omitted when a prefix delta is available.
- Cumulative `text` checkpoint is included on the first event, every **32** sequences, and on non-prefix full replaces.
- Frontend `applyTextDelta` appends `delta` when it is the sole body carrier and `sequence === lastSequence + 1`; otherwise uses `text` as checkpoint.

Measured note: full cumulative `text` on every token was redundant with sequential deltas once the turn registry soft-rejects gaps; checkpoint cadence 32 balances reconnect catch-up vs wire size without a resume protocol yet.

### RC3.3 Phase 4 — minimal turn registry

`src/lib/turn-registry.ts` + `app-store` fields `turnsById` / `activeTurnIdByConversation`:

- Channel text/action/error always update the registry for that turn/conversation, even when the user has navigated away.
- `streamingText` / `agentActions` / `sendError` remain derived live-UI views for the **active** conversation (MessageList unchanged).
- Prefer `delta` append when `sequence === lastSequence + 1`; otherwise use cumulative `text` as checkpoint; soft-reject out-of-order sequences.
- Navigating back to a conversation whose active turn is still `streaming` restores `streamingText` (and actions/error) from the registry.

Channel privacy is unchanged: Text never rides the global `agent-turn` bus. Action / Error / Operation also never ride the global bus (RC3.4).

## What still uses the global bus

- Sync / Conflict **only** when scoped delivery failed (zero successful sync-subscriber sends **and** no invoke Channel success, or missing `conversationId`). Documented residual for tool windows that have not resolved a conversation yet. When any sync subscriber delivers, the global bus must not emit.

Sync payloads include `conversationId`, `surfaceIds`, `toolIds`, and optional `applicationId`. Frontend `shouldApplyAgentTurnSync` skips reloads when the target does not match the active conversation / tool / surfaces. `shouldShowAppConflict` keeps conflict banners conversation-scoped.

## Remaining gaps

- Concurrent multi-turn UI binding beyond one active turn per conversation
- Drop cumulative `text` entirely once reconnect snapshot / resume from last sequence exists
- Reconnect snapshot / resume from last sequence
- Tool windows without a resolvable `surf-{toolId}` conversation still rely on residual global Sync
- Queue-drain progress UI without global private payloads (Action/Error/Operation are now silent on drain)
- Desktop verification of cross-window Sync skip (Application A must not reload B)
- Dead sync Channels for a **closed** window can linger until the next emit fails a send (same-window remounts now replace by label and no longer absorb Sync)

## Related

- [TRUE_STREAMING_RUNTIME.md](./TRUE_STREAMING_RUNTIME.md)
- [MULTIWINDOW_CONCURRENCY.md](./MULTIWINDOW_CONCURRENCY.md)
- [BRANCH_SNAPSHOT_REPLAY.md](./BRANCH_SNAPSHOT_REPLAY.md)
