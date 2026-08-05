# Partial Update Port Provenance

**Product:** Coreside  
**Access date:** 2026-08-04  
**Partial Update SHA-256:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607`  
**MIT:** Copyright (c) 2026 Phil Holden — see `THIRD_PARTY_NOTICES.md`

| Partial Update source | Behavior | Classification | Coreside destination | Status | Gap |
| --- | --- | --- | --- | --- | --- |
| `src/index.ts` UpdateStreamParser | Incremental protocol buffer + dispatch completed units before model ends | Direct translated / adapted | `src-tauri/src/runtime_v2/streaming.rs` (`NdjsonFrameParser`) + `preview_transaction.rs` (`PreviewSurface` paint) | Partial — live NDJSON → Channel preview + speculative surface paint; turn-end durable apply | Live durable apply; JSON-blob progressive split |
| `src/index.ts` runModel loop | Chunk → parse → broadcast while generating | Behavioral reimplementation | `ai/auto.rs` + `message_cmds` + Channel (RC3.2–3.4) | Partial — live text + progressive op preview + `PreviewSurface` paint + turn registry | Delta efficiency; reconnect snapshot; desktop E2E |
| `src/index.ts` withQueueMutation | Serialized queue mutations, one active | Substantially adapted | `runtime_v2/queue.rs` + `ConversationQueue.tsx` + `subscribe_conversation_queue` Channel | Partial — backend + scoped Channel UI + 20s reconcile | Full withQueueMutation UI parity; desktop E2E |
| `src/index.ts` broadcast filter | Client-metadata filtered delivery | Behavioral reimplementation | Channel / `subscribe_conversation_sync` / turn subscribers | Partial — private kinds Channel-only; Sync/Conflict prefer conversation-scoped subscribers; preview overlay clears require conversationId for tool/surface targets | Residual global Sync when no subscriber; queue-drain Action/Error/Operation dropped; multi-window desktop **not_run** |
| `src/index.ts` registerFork / snapshotForFork | Branch indexing + chat snapshot clone | Substantially adapted | `runtime_v2/branch.rs` + `ConversationHistory.tsx` | Partial UI | Message-picker branch points |
| `src/index.ts` replay pacing (+ `debug.ts` inspection) | Chronological paced history replay | Behavioral reimplementation | History Replay tab + `turn_timeline` / `list_turn_timeline_cmd` | Partial — paced read-only timeline or transaction stepper (Unit Verified inspection; not re-execution) | Full PU stream/token replay; desktop E2E |
| Form → model | Structured fields continue the turn | Behavioral reimplementation | Typed `StructuredUserInput` + ledger seal + provider text flatten | **Unit Verified** — delimiter not authority | Desktop E2E; full native multimodal desktop chat **not_run** |
| Marker paths | Target regions by path | Behavioral reimplementation | Typed surface/component IDs | Existing | No CSS selectors |
| HTML/JS/CDN/iframe | Generated execution | **Rejected** | N/A | Rejected | — |

Machine-readable mirror: `reports/partial-update-port-map.json`.
