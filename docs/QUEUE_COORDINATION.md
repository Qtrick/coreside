# Queue Coordination (RC3.3 Phase 9)

**Product:** Coreside  
**Status:** Conversation-scoped Channel delivery for queue mutations — Unit Verified  
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
| `subscribe_conversation_queue` Channel | Primary — delivered only to Channels registered for that `conversationId` after enqueue / activate / cancel / remove / complete / requeue snapshot |
| Focus / visibility | Reconciliation when the window becomes active |
| 20s poll | Low-frequency missed-event safety while queue work or send is in flight |

Event kinds (camelCase on the wire):

- `itemAdded`
- `itemActivated`
- `itemCancelled`
- `itemCompleted`
- `queueSnapshot` (requeue / catch-all state reshape)

Payload always includes `conversationId` (and optional `itemId`).

## Privacy

Queue metadata is **not** on the process-wide bus. `emit_queue_changed` sends only to `AppState.queue_subscribers` for the matching conversation. Main-window ACL only (`coreside-main-default`); tool windows do not get this command. Frontend still applies `isQueueEventForConversation` as defense in depth. Failed Channel sends drop dead subscribers.

## Not claimed

- Full Partial Update `withQueueMutation` UI parity
- Removal of all polling
- Desktop E2E proof of multi-window queue refresh
- Explicit Rust unregister command (unmount clears JS handler; dead Channels cleaned on next emit)

## Related

- [PARTIAL_UPDATE_PORT_PROVENANCE.md](./PARTIAL_UPDATE_PORT_PROVENANCE.md)
- [SCOPED_TURN_STREAMING.md](./SCOPED_TURN_STREAMING.md)
- `src/components/chat/ConversationQueue.tsx`
- `src-tauri/src/runtime_v2/queue.rs`
