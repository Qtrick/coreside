# Declarative Partial Updates Crosswalk — 2026-08-09

Architectural reference mapping Chrome/WICG Declarative Partial Updates → Partial Update product → Coreside secure analogues.

**Primary sources**

- Chrome blog: https://developer.chrome.com/blog/declarative-partial-updates (2026-05-19)
- WICG: https://github.com/WICG/declarative-partial-updates (`patching-explainer.md`, `preserve-explainer.md`, `route-matching-explainer.md`, `dynamic-markup-revamped-explainer.md`)
- Partial Update archive SHA `8666c226…` at `.reference/partial-update/partialupdate-main`
- Coreside active tree (Tauri: macOS WKWebView / Windows WebView2 / Linux WebKitGTK)

**Critical portability rule**

Do **not** make Chrome experimental `<template for>` / processing-instruction APIs a Coreside runtime dependency. Coreside implements the valuable *semantics* with typed operations, Rust validation, PreviewTransaction, capability packs, and revisions.

---

## Out-of-order / interleaved patching

### Chrome/WICG concept
Primitive: `<template for="name">` targeting `<?marker>` / `<?start>`/`<?end>` placeholders. Intent: stream patches for different targets interleaved without waiting for a full document. Restrictions: same parent / tree-scope; body-level templates are powerful. Status: experimental (Chrome 148+ flag).

### Partial Update implementation
WebSocket-delivered HTML bodies applied via custom JS + unpkg `template-for-polyfill`. Interleaving approximated by sequential complete HTML updates, not native HTML parser streaming. Unsafe: model HTML executes in the main document.

### Coreside secure analogue
`ProgressiveOpsParser` (`coreside.ops.v1` start/op/complete) → `PreviewTransaction` → Channel-scoped `PreviewSurface` → ToolCanvas overlay → durable commit via validated transaction. Surface/conversation/turn ownership enforced in Rust.

**Source files:** `src-tauri/src/runtime_v2/progressive_ops.rs`, `preview_transaction.rs`, `src-tauri/src/commands/message_cmds.rs`, `src/lib/preview/surface-overlay.ts`, `src/components/tool-canvas/ToolCanvas.tsx`

**Evidence:** Unit Verified for speculative paint path. Journey 19 Desktop evidence is valid only when `reports/progressive-preview-results.json` matches the current source fingerprint on a clean tree — dirty-tree runs are development evidence only.

**Gap / next:** Interleaved multi-surface Desktop journey if product needs A→B→A paint proof beyond unit coverage.

---

## Target scoping and security boundaries

### Chrome/WICG
Same-parent / same shadow-tree restrictions limit where patches apply.

### Partial Update
CLIENT_PROPS path routing; SERVER_PROPS include/exclude clients. No protected chrome boundary — model can restyle the prompt box if markers allow.

### Coreside
Conversation + turn + Channel + `surfaceId` / `toolId` / application ownership; protected resource IDs; tool windows capability-isolated; generated surfaces never receive raw Tauri IPC.

**Evidence:** Unit + capability audits; Journey 14 stream eavesdropping denial.

**Gap:** Continue Kernel sensitive-command defense-in-depth (main-window label checks added in P0.4).

---

## Placeholder / pending / committed lifecycle

### Chrome/WICG
`<?start>` temporary content → incremental `<template for>` → final content (`<?end>` optional).

### Partial Update
Loading markers + successive template patches; no formal speculative vs durable DB phase.

### Coreside
Pending turn → speculative Preview badge → final validation → durable commit / rollback. Intermediate preview state must not write SQLite.

**Evidence:** Journey 19 asserts paint-before-completion on Desktop. Cancel without durable SQLite writes is Unit Verified (`PreviewTransaction`); Journey 20 Desktop cancel is complementary and must not claim unit guarantees by itself.

---

## Repeated patches to one target

### Chrome/WICG
Multiple templates for the same marker; later markers continue the stream.

### Partial Update
Re-emit markers after append; prompt-driven.

### Coreside
Multiple `op` frames against the same surface within one PreviewTransaction before `complete`.

**Evidence:** Unit; Journey 19 single meaningful paint; multi-op Desktop companion optional.

---

## Content revisions / avoiding overwrite

### Chrome/WICG
Proposed `contentrevision` attribute to skip identical content.

### Partial Update
Not a hard revision CAS model.

### Coreside
Existing `base_revision` / `current_revision` in `runtime_v2/patch.rs` + transaction commit checks. Stale preview must not overwrite N+1.

**Do not** create a second revision system.

**Evidence:** Unit revision conflict tests; Desktop stale-commit journey remains a candidate.

---

## Preservation

### Chrome/WICG
`preserve` attribute — island-style keep element identity across range replacement (draft).

### Partial Update
Prompt convention (“preserve markers”); not engine-enforced typed policies.

### Coreside
Richer `PreservationPolicy` enum in `runtime_v2/preservation.rs` (Replace, PreserveInstance, PreserveState, PreserveUserInput, PreserveMediaState, PreserveScroll, PreserveFocus, PreserveSelection, PreserveIfCompatible, ResetExplicitly).

**Evidence:** Unit; Desktop elevation candidate after Journey 19/CS-PACKS.

---

## Safe vs unsafe markup APIs

### Chrome/WICG
Explicit safe vs unsafe setters; `runScripts` only on unsafe; sanitizer + Trusted Types integration.

### Partial Update
Effectively always-unsafe: `innerHTML` + script activation; `sanitizeModelOutput` is a no-op.

### Coreside
**Intentionally rejected:** raw model HTML/JS/CSS/CDN/`runScripts`/unsafe flags in the trusted surface protocol (`PU-HTML-RESP`, `PU-JS`, `PU-CDN`, `PU-FORM-ROUTE`, `PU-SCOPED-CSS`).

Trusted declarative components only. No `unsafe: true` operation flag.

---

## Batching / atomic updates

### Chrome/WICG
Future batching extension for simultaneous patches.

### Partial Update
SPLIT_MESSAGE multi-update turns; not transactional rollback to SQLite.

### Coreside
PreviewTransaction + durable application transactions (all-or-nothing, rollback on cancel/validation failure).

**Do not** add a second batch layer.

---

## Route / navigation lifecycle

### Chrome/WICG
Declarative route matching; pending/loading/committed phases (`route-matching-explainer.md`).

### Partial Update
SPA-style shell + path CLIENT_PROPS; fork URLs.

### Coreside
`navigate_route_cmd`, route state, generated application surfaces — **research/audit only in P0.4**. Future candidate: pending route states + preserve across safe transitions. Must not displace progressive-preview proof.

---

## Streaming sanitization / remote patch fetch / CSP / scripts

| Topic | Chrome | Partial Update | Coreside |
| --- | --- | --- | --- |
| Streaming sanitization | Sanitizer on safe APIs | None | Typed ops; no HTML stream into protected DOM |
| `patchsrc` remote fetch | Proposed; CSP/CORS | CDN scripts via model | Rejected for generated UI |
| CSP | Browser CSP applies | Weak / product-open | `script-src 'self'`; style `'unsafe-inline'` + optional Google Fonts — residual |
| Script execution | `runScripts` unsafe opt-in | Default product behavior | Forbidden in protected webview |

---

## Matrix summary

| Concept | Already implemented securely? | Evidence need | Intentionally rejected? |
| --- | --- | --- | --- |
| Interleaved patches | Yes (ops frames) | Desktop multi-surface optional | — |
| Target scoping | Yes (stronger) | Keep testing | — |
| Placeholder lifecycle | Yes (PreviewTransaction) | Journey 19 Desktop + Journey 20 cancel (dirty = development-only) | — |
| Revisions | Yes | Stale-commit Desktop candidate | — |
| Preservation | Yes (richer) | Desktop candidate | — |
| Atomic batch | Yes (transactions) | Covered by commit/rollback | — |
| Raw HTML/JS/CDN | — | — | **Yes** |
| Chrome `<template for>` runtime | — | — | **Yes** (as dependency) |
| Route declarative matching | Partial / future | Research only | Not rejected; deferred |

---

## Relationship note

Partial Update closely copies Chrome’s **marker + template-for vocabulary** but transports complete HTML over WebSockets and executes it. Coreside keeps the **product lesson** (conversation → precise in-place persistent UI) and replaces the mechanism with a cross-webview-safe trusted protocol.
