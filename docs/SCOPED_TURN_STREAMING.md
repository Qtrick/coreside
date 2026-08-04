# Scoped Turn Streaming (RC3.2 Phase 3 → RC3.3 Phase 4)

**Product:** Coreside  
**Status:** Interactive Channel + minimal frontend turn registry landed — not complete  
**Access date:** 2026-08-03

## Problem

`app.emit("agent-turn", …)` is process-wide. Any webview with `core:event:default` (including tool windows) can receive another conversation’s assistant text. Client-side filtering is defense in depth, not authorization.

## Design (this slice)

Interactive `send_message` takes a required Tauri `Channel<AgentTurnEvent>` (`onEvent` from the frontend).

| Kind | Channel present (interactive send) | Channel absent (queue drain) |
| --- | --- | --- |
| `text` | Channel only — **never** global emit | Skipped (privacy) |
| `action` / `error` / `operation` | Channel only | Temporary `app.emit` degradation |
| `sync` / `conflict` | Channel **and** global `app.emit` | Global `app.emit` (multi-window refresh) |

Frontend:

- `api.sendMessage` creates `@tauri-apps/api/core` `Channel` and passes `onEvent`.
- `app-store.sendMessage` applies Channel text/action/error/operation for that send (conversation-id check retained).
- `listenAgentTurn` remains for Sync/Conflict (and must ignore private kinds).
- Tool-window bootstrap already ignores text/action/error/operation — keep that (eavesdropping denial).

`AgentTurnEvent::Text` may include optional `turnId`, `sequence`, and `delta` (chunk) while `text` stays the cumulative checkpoint.

### RC3.3 Phase 4 — minimal turn registry

`src/lib/turn-registry.ts` + `app-store` fields `turnsById` / `activeTurnIdByConversation`:

- Channel text/action/error always update the registry for that turn/conversation, even when the user has navigated away.
- `streamingText` / `agentActions` / `sendError` remain derived live-UI views for the **active** conversation (MessageList unchanged).
- Prefer `delta` append when `sequence === lastSequence + 1`; otherwise use cumulative `text` as checkpoint; soft-reject out-of-order sequences.
- Navigating back to a conversation whose active turn is still `streaming` restores `streamingText` (and actions/error) from the registry.

Channel privacy is unchanged: Text never rides the global `agent-turn` bus.

## What still uses the global bus

- Sync / Conflict for multi-window surface reload and conflict UX
- Queue-drain Action / Error / Operation (temporary; no subscriber Channel)

Sync payloads include `conversationId`, `surfaceIds`, `toolIds`, and optional `applicationId`. Frontend `shouldApplyAgentTurnSync` skips reloads when the target does not match the active conversation / tool / surfaces. `shouldShowAppConflict` keeps conflict banners conversation-scoped.

## Remaining gaps

- Concurrent multi-turn UI binding beyond one active turn per conversation
- Delta-only IPC (drop cumulative `text` on the wire when reconnect is solved)
- Reconnect snapshot / resume from last sequence
- Remove Sync global emit once Channel + registry cover tool windows
- Queue-drain progress UI without global private payloads
- Desktop verification of cross-window Sync skip (Application A must not reload B)

## Related

- [TRUE_STREAMING_RUNTIME.md](./TRUE_STREAMING_RUNTIME.md)
- [MULTIWINDOW_CONCURRENCY.md](./MULTIWINDOW_CONCURRENCY.md)
