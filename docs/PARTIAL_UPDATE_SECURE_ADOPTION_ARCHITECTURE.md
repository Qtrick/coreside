# Partial Update Secure Adoption Architecture

**Product:** Coreside  
**Access date:** 2026-08-03  
**Status:** Architecture for RC3 secure parity — **not** an implementation-complete claim  
**Public beta / Local-first / Hosted AI:** all **NOT READY**

Companion reports: `reports/partial-update-secure-parity-matrix.json`, `reports/partial-update-system-map.json`, `reports/partial-update-current-gap-audit.json`.  
Re-audit narrative: [PARTIAL_UPDATE_2026_REAUDIT.md](./PARTIAL_UPDATE_2026_REAUDIT.md).

## Goal

Capture Partial Update’s highest-value behaviors — persistent generative state, targeted updates, structured interaction loops, queue/continuity, progressive apply — as **Coreside-native trusted systems**.

Never import Partial Update’s execution model into the protected webview.

## Non-negotiable rejections

| Partial Update behavior | Why rejected | Coreside boundary |
| --- | --- | --- |
| Model returns HTML as UI | Scriptable DOM in protected webview | Schema-validated operations + registered components |
| Arbitrary JS / CDN scripts | RCE-class + spend loops | Capability packs only; no package/CDN install |
| Hidden iframe `c/:chatId/form` | Extra HTTP surface + client secrets in browser | In-process `submitToAgent` / registered actions |
| Full chrome / Wikipedia rewrite | Can destroy protected UI | Protected core immutable |
| Cloudflare DO / D1 / AI Gateway as core | Wrong product runtime | Tauri + SQLite; BYOK; Hosted AI separate |
| Multiuser private routing via model cooperation | Privacy fails open | Deferred collaboration |

## Target architecture (secure equivalents)

```text
User / structured form / registered action
        │
        ▼
React ToolRenderer / InlineSurface  (no model HTML)
        │
        ▼
Tauri command / Application Kernel
        │
        ├─ validate scope, pack, revision, risk, policy
        ├─ context ledger (model_context_only) ──► MUST inject into prompt
        ├─ agent queue / attachments (atomic commit)
        └─ AiProvider::stream (target) ──► NdjsonFrameParser
                    │
                    ▼
         complete validated operation frames
                    │
                    ▼
         preview → approval when required → transactional apply
                    │
                    ▼
         preservation / versions / branch / replay / inspector (trusted UI)
```

### Streaming (target vs current)

| Layer | Target | Current evidence |
| --- | --- | --- |
| Provider | `stream` on `AiProvider` with cancel + byte limits | `chat` only |
| Transport | Chunks → `NdjsonFrameParser` | Parser unused for live provider bytes |
| UX text | Real `assistant.delta` from provider | `emit_text_fluidly` fake post-hoc typing |
| Ops | Apply only complete validated frames | Cannot progressively apply from provider stream |

### Forms / events (target vs current)

| Layer | Target | Current evidence |
| --- | --- | --- |
| UI | Trusted form components | Present (`submitToAgent`) |
| Persistence | Context ledger entry with values | `appendContextLedger` called |
| Model visibility | Ledger entries in `build_agent_prompt` | **Not injected**; only summary string sent |
| Transport | No iframe | Correctly avoided |

### Continuity surfaces (target vs current)

| Surface | Target | Current |
| --- | --- | --- |
| Branch | Create/list/switch in chat UI | Rust + API; **UI disconnected** |
| Replay | Paced player on trusted transactions | APIs; **UI disconnected** |
| Queue | Visible queue + cancel | Rust + API; **UI disconnected** |
| Inspector | Redacted Developer Mode panel | Toggle only; **UI disconnected** |

## Trust boundaries

1. **Rust / Application Kernel** is authoritative for persistence, risk, permissions, and protected resources.
2. **Frontend validation** improves UX; it is not a security boundary.
3. **Provider adapters** enforce connect/read/total timeouts, decompressed byte ceilings, cancellation, and safe error truncation (`http_limits` work started; streaming backpressure still required when stream lands).
4. **Capability packs** are bundled/allowlisted; unknown components fail closed.
5. **Revisions** prevent stale overwrites; incomplete JSON never applies.
6. **Project isolation** on ledgers, FTS, and surface queries remains mandatory.

## Adoption priority (product value × security)

**P0 (correctness / honesty blockers for generative parity)**

1. True provider streaming + remove fake-streaming as “streaming complete”.
2. Forms ledger → prompt injection (structured values model-visible).
3. Keep HTML/JS/CDN/iframe rejection test-backed.
4. Attachment message-commit atomicity and provider response bounds (RC3 dependency order).

**P1 (high-value secure parity)**

5. Patch readiness / preservation under live frames.
6. Branch / replay / inspector / queue **UI** completion on existing backends.
7. Event loop / subscription local audiences without `window.partialupdates`.

**P2+**

8. Pack breadth (svg/math/canvas/code) within declarative limits.
9. Silent updates / multi-message polish with evidence.
10. Collaboration / cloud auth deferred.

## Mapping to RC3 dependency order

This architecture feeds **phase 2** (re-audit) and constrains later phases:

- Phases **3–11**: data integrity / profile / package hardening (do not weaken for generative flexibility).
- Phases **12–13**: provider bounds + typed provider-neutral messages (foundation for stream).
- Later phases (14–18): true streaming, secure frames, forms/events, branch/replay/inspector.

See [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md).

## Verification required before any “complete” claim

- Desktop journey: form submit → model cites structured values → targeted surface update.
- Desktop journey: provider tokens appear before full completion (not post-hoc typing).
- Desktop journey: branch create/switch; queue cancel; replay pace; inspector open with redaction.
- Adversarial: HTML/script/CDN in model output rejected; iframe form route absent.
- Packaged + E2E evidence recorded; dirty tree never labeled release-ready.

## Explicit non-goals

- Copying Partial Update source into Coreside.
- Claiming July `complete_but_unverified` rows as release gates.
- Public multiuser collaboration in consumer RC3.
- Hosted AI readiness claims.
