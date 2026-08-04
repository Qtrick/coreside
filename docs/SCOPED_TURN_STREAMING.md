# Scoped Turn Streaming (RC3.2 Phase 3)

**Product:** Coreside  
**Status:** Interactive Channel slice landed — not complete  
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

## What still uses the global bus

- Sync / Conflict for multi-window surface reload and conflict UX
- Queue-drain Action / Error / Operation (temporary; no subscriber Channel)

## Remaining gaps

- Full turn registry (concurrent turns, background UI binding)
- Delta-only efficiency (UI still paints from cumulative `text`)
- Reconnect snapshot / resume from last sequence
- Remove Sync global emit once Channel + registry cover tool windows
- Queue-drain progress UI without global private payloads

## Related

- [TRUE_STREAMING_RUNTIME.md](./TRUE_STREAMING_RUNTIME.md)
- [MULTIWINDOW_CONCURRENCY.md](./MULTIWINDOW_CONCURRENCY.md)
