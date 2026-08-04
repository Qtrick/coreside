# Partial Update Port Provenance

**Product:** Coreside  
**Access date:** 2026-08-03  
**Partial Update SHA-256:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607`  
**MIT:** Copyright (c) 2026 Phil Holden — see `THIRD_PARTY_NOTICES.md`

| Partial Update source | Behavior | Classification | Coreside destination | Status | Gap |
| --- | --- | --- | --- | --- | --- |
| `src/index.ts` UpdateStreamParser | Incremental protocol buffer + dispatch completed units before model ends | Direct translated / adapted | `src-tauri/src/runtime_v2/streaming.rs` (`NdjsonFrameParser`) + `preview_transaction.rs` | Partial — live NDJSON → Channel preview; turn-end apply | Live durable apply; JSON-blob progressive split |
| `src/index.ts` runModel loop | Chunk → parse → broadcast while generating | Behavioral reimplementation | `ai/auto.rs` + `message_cmds` + Channel (RC3.2/3.3) | Partial — live text + progressive op preview | Delta efficiency; turn registry; reconnect snapshot; surface paint |
| `src/index.ts` withQueueMutation | Serialized queue mutations, one active | Substantially adapted | `runtime_v2/queue.rs` + `ConversationQueue.tsx` + `subscribe_conversation_queue` Channel | Partial — backend + scoped Channel UI + 20s reconcile | Full withQueueMutation UI parity; desktop E2E |
| `src/index.ts` broadcast filter | Client-metadata filtered delivery | Behavioral reimplementation | Channel / turn subscribers | Partial — Channel for interactive text | Queue-drain Action/Error may still use global; Sync/Conflict remain global (not text) |
| `src/index.ts` fork/snapshot | Branch indexing | Substantially adapted | `runtime_v2/branch.rs` + `ConversationHistory.tsx` | Partial UI | Message-picker branch points |
| `src/debug.ts` / replay pacing | Chronological replay | Behavioral reimplementation | History Replay tab | Partial — transaction list | Full paced player |
| Form → model | Structured fields continue the turn | Behavioral reimplementation | Typed `StructuredUserInput` + ledger seal + provider text flatten | **Unit Verified** — delimiter not authority | Native multimodal provider parts; desktop E2E |
| Marker paths | Target regions by path | Behavioral reimplementation | Typed surface/component IDs | Existing | No CSS selectors |
| HTML/JS/CDN/iframe | Generated execution | **Rejected** | N/A | Rejected | — |

Machine-readable mirror: `reports/partial-update-port-map.json`.
