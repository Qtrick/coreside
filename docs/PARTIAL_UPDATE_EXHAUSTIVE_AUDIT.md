# Partial Update Exhaustive Audit

**Access date:** 2026-07-18  
**Product:** Coreside (analysis target)  
**Reference:** Partial Update (MIT, Phil Holden) — research source only

## Archives

| Archive | SHA-256 | Size | Extract |
|---|---|---|---|
| Partial Update Main.zip | `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` | 161880 | `.reference/partial-update/partialupdate-main` |
| Coreside Chat AI.zip | `196696bd77f286649e9c73f5a5e0dd02dbaf20d59322c2c117f735eaf5392535` | 2228890 | `.reference/coreside-uploaded/coreside-main` |

Active repository: `/Users/qunyingfan/Coreside` (branch `main`). Uploaded Coreside ZIP is a subset of the active tree (372 common files; 0 only-in-upload; active extras are local `.cursor` rules, `.env`, crawl4ai caches).

Partial Update GitHub `main` HEAD `91c27c31` (2026-06-27) matches the archive source tree; later commits are README/video-link only.

## Architecture (Partial Update)

- **Runtime:** Cloudflare Worker + Durable Object (`PartialUpdate`), hibernatable WebSockets, D1 for auth.
- **Storage:** DO SQL for chat/LLM/visible history; fork index; rate-limit state; LLM queue.
- **Protocol:** Delimiter `PpqUtcLGQdYN4oqc` with `SPLIT_MESSAGE`, `SERVER_PROPS`, `CLIENT_PROPS`, `BODY` sections. Bodies are raw HTML/JSON (not escaped).
- **Client updates:** `<template for="path">` + `<?marker|/start|/end>` processing instructions; polyfill/browser DPU.
- **Forms:** POST to hidden iframe `c/:chatId/form` with `clientId`/`clientSecret`/`path`.
- **Streaming:** Provider stream → `UpdateStreamParser` → complete frames applied progressively.
- **Multiuser:** Client IDs, include/exclude routing, private model-only messages, shared DO state.
- **Fork/Replay/Debug:** Stable fork IDs, read-only fork pages, replay pacing options, debug HTML views.
- **Queue/Limits:** Queue max 5; browser/IP prompt & chat rate limits; signed browser cookie.
- **Auth:** better-auth (Google/GitHub/email), roles (admin/developer/chat/view), optional.

## Security weaknesses (Partial Update)

1. Arbitrary model-generated JavaScript and CDN script injection.  
2. Form/script loops can burn inference and Worker spend (documented by author).  
3. Full-document rewrite can break host chrome.  
4. Multiuser private routing relies on model cooperation (demo shows alignment failure).  
5. Secrets in `.env` / keychain scripts — expected for their stack, unsafe to copy into Coreside webview.

## Strongest product insights (adopt safely)

1. Treat generated UI as **persistent application state**, not chat ephemera.  
2. **Stable targeting** + multiple instances prevent overwrite.  
3. **Update only what changed**; preserve interactive state.  
4. **Structured user events** (forms/games) beat free-text for app loops.  
5. **Silent updates** and multi-region updates in one turn.  
6. Replay/debug/queue/rate-limits are first-class.

## Coreside mapping

Coreside Runtime V2 implements schema version `"2"` with validated operations, surfaces, patches, inline chat surfaces, capability packs (no CDN), typed events with loop controls, transactions/undo, branching/snapshots, request queue, and developer diagnostics — while preserving schema v1 `tool_change`.

See `docs/PARTIAL_UPDATE_FEATURE_MATRIX.md` and `docs/GENERATIVE_INTERFACE_RUNTIME_V2.md`.
