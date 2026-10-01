# Partial Update ↔ Coreside Parity Matrix

Source archive inspected: Partial Update `src/index.ts` + `initialPrompt.md` (extracted for analysis).
Coreside paths refer to the live repository.

Status legend: **Equivalent** · **Partial** · **Implemented differently** · **Rejected (security)** · **Deferred**

| Mechanism | Partial Update | Coreside | Status | Evidence | Remaining Gap |
|---|---|---|---|---|---|
| Serialized queue mutation | `withQueueMutation` (`index.ts`) | `runtime_v2/queue.rs` exclusive activate | Partial | SQLite IMMEDIATE enqueue/activate | Multi-window desktop E2E |
| LLM queue (cap 5) | `enqueueLlmRequest` / drain | `MAX_QUEUED_TURNS=5`, `ConversationQueue` | Partial | queue.rs + UI | Drain UX polish |
| Stale active recovery | wall-clock stale clear | `recover_stale_active` + turn journal | Implemented differently (stronger) | queue.rs | Startup recovery evidence |
| Rate limiting | IP/browser cookie permits | Hosted gateway + local breakers | Implemented differently | intentional BYOK | Do not port cookies |
| Prompt limits | 20k chars | `MAX_USER_MESSAGE_CHARS` | Equivalent | limits.rs / message_cmds | — |
| Transport | Hibernatable WebSocket | Tauri Channel IPC | Rejected multiuser WS | intentional | Channel residual paths |
| Client session secrets | clientId+secret in forms | Conversation ownership / keyring | Rejected (security) | intentional | Never embed secrets in surfaces |
| Reconnect | WS backoff reconnect | Channel re-subscribe + catch-up | Partial | conversation-catch-up | Surface rehydrate proof |
| Replay | paced DOM history | `ReplayPlayer` + turn timeline | Partial | read-only | No provider re-execution |
| Hard undo | delete N turns | `undo_transaction` OCC | Partial | transactions.rs | Multi-turn consumer undo |
| Fork snapshot | DO clone + fork index | `runtime_v2/branch.rs` | Partial | branch.rs | Branch UI journey |
| Form submission | iframe POST + secret | `StructuredUserInput` sealed | Implemented differently (secure) | structured_user_input.rs | Rich-form E2E |
| Client-specific updates | SERVER_PROPS include/exclude | Audience on operations | Partial | operations.rs | Multiuser deferred |
| Broadcast routing | filtered WS broadcast | conversation-scoped Channel | Partial | message_cmds | Multi-window proof |
| Marker HTML patches | `<template for>` / markers | Typed `AppOperation` / progressive ops | Rejected (security) | intentional | Declarative ops only |
| Stable IDs | DOM marker paths | surface/component/instance ids | Equivalent (secure) | surfaces.rs | Progressive ID CAS |
| App instances | `ttt/1`, `ttt/2` | `instance_id` + mounts | Equivalent | surfaces / mount_registry | UX clarity |
| Progressive rendering | complete-unit HTML stream | NDJSON progressive ops + preview txn | Partial | progressive_ops.rs | Live durable multi-surface |
| Layout-shift | sized outer container guidance | preservation helpers | Partial | preservation.ts | Streaming placeholders |
| Script cleanup | MutationObserver scripts | No model JS | Rejected (security) | intentional | Capability packs only |
| Auth / roles | Better Auth roles | Keyring + Kernel grants | Implemented differently | intentional | Enterprise RBAC deferred |
| Prompt injection | identity sanitize | research sanitize + pack validation | Implemented differently (stricter) | intentional | Broader boundary tags |
| Model protocol | delimiter HTML protocol | ApplicationPlan + JSON schema ops | Implemented differently (secure) | application_plan.rs | Provider compliance |
| History persistence | DO SQL messages | SQLite + turn journal | Equivalent (local-first) | db | Cross-device deferred |

## Highest-priority remaining gaps

1. Progressive durable apply completeness for multi-surface turns.
2. Reconnect/catch-up proof for declarative surfaces (not only text).
3. Consumer-safe multi-turn undo comparable to Partial Update hard undo.

## Intentional non-goals

Raw HTML/JS/CSS, CDN, iframe forms, client secrets in generated UI, multi-tenant hosted DO assumptions.
