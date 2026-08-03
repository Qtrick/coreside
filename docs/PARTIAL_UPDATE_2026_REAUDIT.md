# Partial Update 2026 Re-Audit (RC3)

**Product:** Coreside  
**Access date:** 2026-08-03  
**Phase:** RC3 — Workstream A (baseline + parity)  
**Public beta:** **NOT READY**  
**Local-first:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Purpose

Re-audit the immutable Partial Update reference against **current active Coreside source**, correct stale July 2026 status claims, and record secure adoption targets without claiming features complete without evidence.

This document supersedes July Coreside status interpretations in:

- `docs/PARTIAL_UPDATE_EXHAUSTIVE_AUDIT.md`
- `docs/PARTIAL_UPDATE_FEATURE_MATRIX.md`
- `docs/PARTIAL_UPDATE_FINAL_GAP_AUDIT.md`
- `docs/PARTIAL_UPDATE_FILE_ANALYSIS.md`
- `reports/partial-update-*.json` (July)

July files remain useful as **Partial Update mechanism catalogs**. Their **Coreside completeness claims are stale**.

## Archives and active source

| Artifact | SHA-256 | Notes |
| --- | --- | --- |
| Coreside archive (`Coreside main.zip`) | `ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228` | present_hash_match |
| Partial Update archive | `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` | **Byte-identical** to July extract |
| Active commit | `41fd4595db420eac8b1c5cc30723f1dd377608f4` | Dirty worktree |
| Source fingerprint | `3306d5de459be8d661c52e0101b70d956c2218f841bacafdbedb6ebb6acf9f41` | development-dirty |
| `package-lock.json` | `f32e3db1ed083c4c69903b536cb15edbb1ba616e8f8affd75c113cd7004e3be6` | |
| `src-tauri/Cargo.lock` | `4b118413c6104abc1eab2df0455e2d2218df058c59c075866191c677709d0729` | |

Extract: `.reference/partial-update/partialupdate-main` (29 source files; ~772 KB including generated Worker stubs).

Baseline companion: [CURRENT_SOURCE_BASELINE.md](./CURRENT_SOURCE_BASELINE.md).

Machine-readable outputs:

- `reports/partial-update-file-manifest-current.json`
- `reports/partial-update-secure-parity-matrix.json`
- `reports/partial-update-system-map.json`
- `reports/partial-update-current-gap-audit.json`

## Partial Update architecture (reference only)

Partial Update is a Cloudflare Worker + Durable Object chat that treats the **document as mutable application state**:

1. Model emits delimiter protocol (`PpqUtcLGQdYN4oqc`) with optional server/client props and **raw HTML bodies**.
2. Client applies `<template for="path">` and `<?marker>` updates (Declarative Partial Updates–style).
3. Forms POST to a hidden iframe (`c/:chatId/form`) and re-enter the LLM queue.
4. Provider **streams** into an incremental parser; complete frames apply progressively.
5. Forks, replay pages, debug HTML history, queue (max 5), and browser/IP rate limits are first-class.
6. Optional better-auth (Google/GitHub/email) + D1 roles; multiuser include/exclude routing.

**Author-documented hazards:** model JS loops burn inference; CDN scripts; full-chrome rewrite; multiuser privacy relies on model cooperation.

## Secure Coreside stance (unchanged product law)

Adopt product insights; **reject** execution model:

| Reject | Coreside replacement |
| --- | --- |
| Arbitrary HTML responses | Trusted declarative components + schema v2 ops |
| Arbitrary JS / CDN | Bundled capability packs only |
| Hidden iframe form routes | In-process `submitToAgent` / registered actions |
| Cloudflare DO / AI Gateway as identity | Local Tauri + SQLite; provider-neutral BYOK; Hosted AI separate and **Not ready** |
| Full protected-chrome rewrite | Protected core immutable; user-editable zones only |

## Evidence-backed Coreside gaps (2026-08-03)

### 1. True streaming is ABSENT

- `AiProvider` exposes `chat()` only (`src-tauri/src/ai/provider.rs`).
- After a complete provider response, `emit_text_fluidly` in `message_cmds.rs` sleeps between characters to **simulate** typing.
- `runtime_v2/streaming.rs` `NdjsonFrameParser` exists but is **not** fed by live provider byte streams.

July claim that progressive streaming is `implemented_differently` / `complete_but_unverified` is **stale**. Status: **absent**.

### 2. Forms ledger is not injected into prompts

Observed path (`InlineSurface.tsx` / `ToolCanvas.tsx`):

1. `appendContextLedger` stores structured `values` with `model_context_only`.
2. `sendMessage(summary)` sends only a human summary string (e.g. `Surface form submitted (…)`).
3. `message_cmds` prompt construction does **not** call `list_context_ledger`.

Structured form values therefore do **not** become model-visible context. July “forms implemented differently” overstates product behavior. Status: **partial** (write path only).

### 3. Branch / replay / inspector / queue UI largely disconnected

| Capability | Backend | UI |
| --- | --- | --- |
| Branch / fork | `runtime_v2/branch.rs` + `api.branchConversation` / `listBranches` | No `src/components` callers |
| Queue | `runtime_v2/queue.rs` + list/cancel APIs | No queue UI components |
| Replay | Transaction list APIs | No paced replay player |
| Inspector | Diagnostics commands; Developer Mode toggle | No inspector panel |

July matrix “missing” vs “implemented” rows for these are inconsistent; treat product status as **disconnected** unless UI + journey evidence exists.

### 4. Backend present but July “missing_high_value” stale for some items

Inline surfaces, patches, packs, and transactions have substantial Rust/React foundations. Do **not** treat them as absent — and do **not** treat them as release-complete. Prefer: **partial** / **implemented-unverified** / **disconnected** per matrix.

## Strongest Partial Update insights to keep

1. Generated UI as **persistent application state**, not chat ephemera.
2. **Stable targeting** + multiple instances (no overwrite by name).
3. Update **only what changed**; preserve interactive state.
4. **Structured user events** (forms/games) beat free-text for app loops.
5. Silent multi-region updates in one turn.
6. Queue, rate limits, fork/replay/debug as first-class product surfaces — via **trusted** Coreside UI.

## Explicit non-claims

- Public beta **NOT READY**.
- Local-first **NOT READY**.
- Hosted AI **NOT READY**.
- Dirty worktree ≠ release snapshot.
- No feature is `implemented-verified` in this re-audit solely because a type, command, or July checkbox exists.
- HTML / JS / CDN / iframe forms remain **rejected**.

## Related docs

- [PARTIAL_UPDATE_FILE_ANALYSIS_2026.md](./PARTIAL_UPDATE_FILE_ANALYSIS_2026.md)
- [PARTIAL_UPDATE_SECURE_ADOPTION_ARCHITECTURE.md](./PARTIAL_UPDATE_SECURE_ADOPTION_ARCHITECTURE.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) (RC3 phases 1–13)
- [GENERATIVE_INTERFACE_RUNTIME_V2.md](./GENERATIVE_INTERFACE_RUNTIME_V2.md) (foundation; status claims need evidence refresh)
