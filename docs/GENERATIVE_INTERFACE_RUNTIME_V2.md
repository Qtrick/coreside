# Generative Interface Runtime V2

**Product:** Coreside  
**Status:** Foundation shipped (2026-07-18)

## Goals

Capture Partial Update’s best product ideas without copying unrestricted HTML/JS/CDN execution into the protected webview.

## Schema

- **v1** (unchanged): `assistantMessage` + `responseType` + single `toolChange` / `settingsChange` / `tool_use`.
- **v2**: `assistantMessages[]`, `operations[]`, `silent`, `turnId`, transaction groups, base revisions, audience extension points.

Framing for progressive ops: **NDJSON** stream events (`turn.started`, `assistant.delta`, `operation.frame_completed`, …). Incomplete JSON is never applied.

## Core modules

| Module | Path |
|---|---|
| Operations | `src-tauri/src/runtime_v2/operations.rs` |
| Patches | `src-tauri/src/runtime_v2/patch.rs` |
| Surfaces | `src-tauri/src/runtime_v2/surfaces.rs` |
| Transactions | `src-tauri/src/runtime_v2/transactions.rs` |
| Events | `src-tauri/src/runtime_v2/events.rs` |
| Packs | `src-tauri/src/runtime_v2/packs.rs` |
| Streaming | `src-tauri/src/runtime_v2/streaming.rs` |
| Queue | `src-tauri/src/runtime_v2/queue.rs` |
| Branch/Snapshot | `src-tauri/src/runtime_v2/branch.rs` |
| Migration | `src-tauri/migrations/012_runtime_v2.sql` |
| Inline UI | `src/components/chat/InlineSurface.tsx` |

## Conflict handling and isolation

`component.*` ops require matching `baseRevision`. Stale patches fail closed (no silent overwrite). Full `tool.full_replace` remains available as fallback.
`Audience` routing strictly enforces cross-project and cross-chat boundaries, ensuring transactions in Project A never touch surfaces in Project B. `FutureParticipants` records historical intent without mutating live active surfaces.
Branch diffing (`diff_branch_cmd`) computes message deltas and component structural differences without requiring destructive restoration.

## Capability packs

Bundled only: core, forms, svg, charts, code, math, canvas, audio, data, dictation (extension point). Agent cannot install packs or load CDNs.

## Non-goals (this phase)

Public collaboration, cloud auth, arbitrary JS/CDN, Wasm sandbox (researched/deferred), marketplace packs.
