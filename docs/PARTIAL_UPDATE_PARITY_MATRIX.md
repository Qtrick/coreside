# Partial Update ↔ Coreside Parity Matrix

Source archive inspected: Partial Update `src/index.ts` + `initialPrompt.md` (extracted for analysis; sha256 `8666c226…`).
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
| Reconnect | WS backoff reconnect | Channel re-subscribe + catch-up | Partial | conversation-catch-up + turn_journal cursors; Journey 24–25 desktop specs wired | Multi-window Cases H–I; packaged proof |
| Replay | paced DOM history | `ReplayPlayer` + turn timeline | Partial | read-only | No provider re-execution |
| Hard undo | delete N turns | `undo_transaction` OCC | Partial | transactions.rs one/two-turn undo unit; Journey 30 desktop via `undo_transaction_cmd` | Consumer History Undo button (still invoke/API-first) |
| Fork snapshot | DO clone + fork index | `runtime_v2/branch.rs` + `branchConversation` UI | Partial | branch.rs record/revision isolation unit; fork remaps ApplicationPlan tools from conversation transactions + tools.id; Journey 27 desktop (branch Task Tracker isolated) | Packaged fork pages / read-only fork UX |
| Form submission | iframe POST + secret | `StructuredUserInput` sealed | Implemented differently (secure) | structured_user_input + authorize hostile unit; Journey 28 Tic-Tac-Toe submitToAgent (mock fixture stateContracts for lastMove) | Rich multi-field form desktop beyond game.move |
| Client-specific updates | SERVER_PROPS include/exclude | Audience on operations | Partial | operations.rs | Multiuser deferred |
| Broadcast routing | filtered WS broadcast | conversation-scoped Channel | Partial | message_cmds | Multi-window proof |
| Marker HTML patches | `<template for>` / markers | Typed `AppOperation` / progressive ops | Rejected (security) | intentional | Declarative ops only |
| Stable IDs | DOM marker paths | surface/component/instance ids | Equivalent (secure) | surfaces.rs + surface_diff.rs | — |
| Collection incremental update | marker/stream item updates | `data.record_*` + stable record ids | Partial | data.rs preservation tests | Typed collection.* ops deferred |
| App instances | `ttt/1`, `ttt/2` | `instance_id` + mounts | Equivalent | surfaces / mount_registry | UX clarity |
| Progressive rendering | complete-unit HTML stream | NDJSON progressive ops + preview txn | Partial | progressive_ops.rs + e2e seed contracts | Multi-surface progressive durable |
| Application evolution | full HTML rewrite units | ApplicationPlan → surface_diff granular `component.*` | Implemented differently (secure) | surface_diff.rs + compile_plan_against_db; lineage-resolved ApplicationSpec; fail-closed LineageScope (missing binding rejects); lineage_scope_for_plan omits claims for workspace globals | Live provider evidence |
| Layout-shift | sized outer container guidance | preservation helpers | Partial | preservation.ts; Journey 5 shell overflow locally green on HEAD; remote macOS E2E still failing (logs blocked without gh auth; artifacts now uploaded on failure) | Remote root-cause + streaming placeholders |
| Script cleanup | MutationObserver scripts | No model JS | Rejected (security) | intentional | Capability packs only |
| Auth / roles | Better Auth roles | Keyring + Kernel grants | Implemented differently | intentional | Enterprise RBAC deferred |
| Prompt injection | identity sanitize | research sanitize + pack validation | Implemented differently (stricter) | intentional | Broader boundary tags |
| Model protocol | delimiter HTML protocol | ApplicationPlan + JSON schema ops | Implemented differently (secure) | application_plan.rs | Provider compliance |
| History persistence | DO SQL messages | SQLite + turn journal | Equivalent (local-first) | db | Cross-device deferred |
| Turn admission | DO single-owner | queue + turn journal | Implemented differently (stronger) | queue.rs | Cross-conversation stress |

## Highest-priority remaining gaps

1. Progressive durable apply completeness for multi-surface turns (desktop).
2. Multi-window reconnect Cases H–I and packaged reconnect proof (Journeys 24–25 cover single-window evolution catch-up).
3. Consumer History Undo control (Journey 30 proves `undo_transaction`; UI still API/invoke-first).
4. Approval restart/recovery desktop (Journey 29) + richer structured forms beyond game.move.
5. Packaged / live-provider verification for granular evolve.

## Intentional non-goals

Raw HTML/JS/CSS, CDN, iframe forms, client secrets in generated UI, multi-tenant hosted DO assumptions, DB-less privileged ApplicationPlan → Apply.
