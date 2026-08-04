# Queue Coordination (RC3.3 Phase 9 partial)

**Product:** Coreside  
**Status:** Event-driven queue UI refresh landed — not full Partial Update parity  
**Access date:** 2026-08-03

## Authority

Rust + SQLite own the per-conversation agent request queue (`runtime_v2/queue.rs`):

- Serialized activate (`activate_next` exclusive UPDATE)
- Max queued turns (`MAX_QUEUED_TURNS`)
- Cancel/remove only for `queued` (not active)
- Startup `recover_stale_active` (already present — not claimed as new here)

This is the Coreside adaptation of Partial Update `withQueueMutation`: mutations are authoritative in SQLite, not in the React tree.

## UI update path (this slice)

| Path | Role |
| --- | --- |
| `agent-queue-changed` events | Primary — emitted after enqueue / activate / cancel / remove / complete / requeue snapshot |
| Focus / visibility | Reconciliation when the window becomes active |
| 20s poll | Low-frequency missed-event safety while queue work or send is in flight |

Event kinds (camelCase on the wire):

- `itemAdded`
- `itemActivated`
- `itemCancelled`
- `itemCompleted`
- `queueSnapshot` (requeue / catch-all state reshape)

Payload always includes `conversationId` (and optional `itemId`).

## Remaining P1

**Queue metadata is still process-wide.** Emit uses `app.emit("agent-queue-changed", …)` today. Frontend filters with `isQueueEventForConversation`. Conversation-scoped Channel (or equivalent) delivery for queue metadata is still open — same class of issue as Sync/Conflict on the global bus (queue payloads are not assistant text, but they are still global).

## Not claimed

- Full Partial Update `withQueueMutation` UI parity
- Conversation-scoped queue Channel
- Removal of all polling
- Desktop E2E proof of multi-window queue refresh

## Related

- [PARTIAL_UPDATE_PORT_PROVENANCE.md](./PARTIAL_UPDATE_PORT_PROVENANCE.md)
- [SCOPED_TURN_STREAMING.md](./SCOPED_TURN_STREAMING.md)
- `src/components/chat/ConversationQueue.tsx`
- `src-tauri/src/runtime_v2/queue.rs`
