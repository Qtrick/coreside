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
| Reconnect | WS backoff reconnect | Channel re-subscribe + catch-up | Partial | conversation-catch-up + turn_journal cursors; Journey 24–25 single-window; Journey 26 multi-window mount isolation wired | Packaged process-relaunch reconnect proof |
| Replay | paced DOM history | `ReplayPlayer` + turn timeline | Partial | read-only | No provider re-execution |
| Hard undo | delete N turns | `undo_transaction` OCC | Partial | transactions.rs one/two-turn undo unit; History → Replay **Undo latest change** UI; Journey 30 desktop | Multi-turn tip undo OCC conflict UX polish |
| Fork snapshot | DO clone + fork index | `runtime_v2/branch.rs` + `branchConversation` UI | Partial | branch.rs record/revision isolation unit; fork remaps ApplicationPlan tools from conversation transactions + tools.id; Journey 27 desktop (branch Task Tracker isolated) | Packaged fork pages / read-only fork UX |
| Form submission | iframe POST + secret | `StructuredUserInput` sealed | Implemented differently (secure) | structured_user_input + authorize hostile unit; Journey 28 Tic-Tac-Toe submitToAgent (mock fixture stateContracts for lastMove) | Rich multi-field form desktop beyond game.move |
| Client-specific updates | SERVER_PROPS include/exclude | Audience on operations | Partial | operations.rs | Multiuser deferred |
| Broadcast routing | filtered WS broadcast | conversation-scoped Channel | Partial | message_cmds | Multi-window proof |
| Marker HTML patches | `<template for>` / markers | Typed `AppOperation` / progressive ops | Rejected (security) | intentional | Declarative ops only |
| Stable IDs | DOM marker paths | surface/component/instance ids | Equivalent (secure) | surfaces.rs + surface_diff.rs | — |
| Collection incremental update | marker/stream item updates | `data.record_*` + stable record ids | Partial | data.rs preservation tests; transactions.rs hostile_2 (same-txn create surface → data model → record CRUD) | Typed collection.* ops deferred |
| App instances | `ttt/1`, `ttt/2` | `instance_id` + mounts | Equivalent | surfaces / mount_registry | UX clarity |
| Progressive rendering | complete-unit HTML stream | NDJSON progressive ops + preview txn | Partial | progressive_ops.rs + Journey 19–20; Journey 31 Study Planner create→evolve durable sections; Journey 32 validate-fail→repair→Apply | True multi-surface atomic progressive durable mid-stream still open |
| Application evolution | full HTML rewrite units | ApplicationPlan → surface_diff granular `component.*` | Implemented differently (secure) | surface_diff.rs + compile_plan_against_db; lineage-resolved ApplicationSpec; fail-closed LineageScope; transaction-local creation; hostile create→mutate; Journey 21 Apply via `[data-proposal-apply]` (not preview btn-primary) | Live provider evidence |
| Layout-shift | sized outer container guidance | preservation helpers | Partial | LiveWallpaper fixed sibling of `.app-shell` (Journey 5 wallpaperInsideShell=false); shell no longer `isolation:isolate` so backdrop-filter can sample wallpaper; `-webkit-backdrop-filter` + overlay tokens | Packaged visual WebKit capture pending push |
| Script cleanup | MutationObserver scripts | No model JS | Rejected (security) | intentional | Capability packs only |
| Auth / roles | Better Auth roles | Keyring + Kernel grants | Implemented differently | intentional | Enterprise RBAC deferred |
| Prompt injection | identity sanitize | research sanitize + pack validation | Implemented differently (stricter) | intentional | Broader boundary tags |
| Model protocol | delimiter HTML protocol | ApplicationPlan + JSON schema ops | Implemented differently (secure) | application_plan.rs | Provider compliance |
| History persistence | DO SQL messages | SQLite + turn journal | Equivalent (local-first) | db | Cross-device deferred |
| Turn admission | DO single-owner | queue + turn journal | Implemented differently (stronger) | queue.rs | Cross-conversation stress |

## Highest-priority remaining gaps

1. True multi-surface atomic progressive durable mid-stream apply (Journey 31 covers single-surface multi-section evolve; not separate surface IDs in one txn mid-stream).
2. Packaged process-relaunch reconnect proof (Journey 26 covers multi-window mount isolation in debug e2e).
3. Full process-relaunch approval survival (Journey 29 covers settings remount + second-decide reject; not OS relaunch).
4. Richer structured forms beyond game.move; packaged / live-provider evolve proof.
5. Preview-seed path may mint `surf-*` keys for non-durable paint only (durable apply is DB-lineage + tool_id ownership).
6. Remote GitHub Desktop E2E green requires push of local Journey 21 Apply selector fix (local Journeys 21/5/31/32 pass).

## Intentional non-goals

Raw HTML/JS/CSS, CDN, iframe forms, client secrets in generated UI, multi-tenant hosted DO assumptions, DB-less privileged ApplicationPlan → Apply.
