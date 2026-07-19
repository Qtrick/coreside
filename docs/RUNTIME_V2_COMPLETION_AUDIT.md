# Runtime V2 Completion Audit

**Product:** Coreside  
**Date:** 2026-07-18  
**Prior tests re-run:** `npm run audit:partial-update` (56 features), `npm run test:runtime-v2` (11 passed), `npm run test` (54 passed), `npm run typecheck` (passed)

This audit classifies every Runtime V2 / Partial Update adoption system before the Application Kernel completion phase. Incomplete prior requirements remain in scope.

## Classification key

| Status | Meaning |
|--------|---------|
| `complete_and_verified` | Implemented, wired, tested |
| `complete_but_unverified` | Implemented but not manually acceptance-tested |
| `partial` | Core path works; gaps remain |
| `placeholder` | Types/stubs only |
| `disconnected` | Exists but not used by agent/UI |
| `broken` | Fails under expected use |
| `unsafe` | Security gap |
| `undocumented` | Behavior exists without docs |
| `missing` | Not started |

## Prior Runtime V2 systems

| System | Status | Notes |
|--------|--------|-------|
| Feature matrix / audit docs | complete_and_verified | 56 features; reports JSON present |
| Protocol V2 schema fields | complete_and_verified | `operations`, `silent`, `turnId`, parser schema `"2"`; applied from `message_cmds` |
| Multi-operation transactions | complete_and_verified | create/apply/undo wired; SAVEPOINT rollback; many op types still no-op (see below) |
| Fine-grained patches | complete_and_verified | props/insert/remove + revision checks; unit tests |
| Surface persistence | complete_and_verified | migration 012; tool wrap; definition CRUD |
| Inline surfaces | partial | UI + message scoping in `InlineSurface.tsx`; no persisted-state load API/UI; archive control disabled |
| Progressive NDJSON streaming | complete_but_unverified | Live harvest in `message_cmds` when schema ops empty; unit tests remain |
| Capability packs registry | complete_and_verified | Rust validation + renderer nodes; no CDN; dictation pack separate |
| Typed events / EventBus | complete_but_unverified | Persist + dispatch + hydrate from SQLite; agent/UI path via apply_change bus |
| Subscriptions persistence | complete_but_unverified | create/update/delete write SQLite + EventBus; filters persisted |
| Workspace editable zones | placeholder | `workspace_layouts` table; layout.* ops no-op; no drag UI |
| Branches / snapshots | disconnected | Rust commands + `tauri.ts` wrappers; no chat UI; `chat.branch_create` apply no-op |
| Replay | partial | Transaction list API; no play/pause player UI |
| Developer inspection | partial | Diagnostics store + commands; not auto-recorded on turns; no Developer Mode panel |
| Request queues | complete_but_unverified | `send_message` enqueues when conversation already active |
| Security adversarial (packs/ops) | partial | Protected IDs, unknown-op reject, pack allowlist, SVG attr filter; no dedicated adversarial suite; many mutating ops silently no-op |
| Accessibility at schema | complete_but_unverified | DOM `verifySurfaceElement` on inline surfaces; schema-level a11y still limited |
| Performance budgets | partial | Tree depth/count and related limits in `limits.rs` only |
| Setting.* / layout.* ops | placeholder | Known in schema; apply records no-op |
| Undo create/delete | partial | Undo restores prior definitions from snapshot only; no create/delete reversal |
| Dictation pack | placeholder | Pack + disabled `dictationButton` node; extension point only |
| Wasm sandbox | missing | Explicitly deferred |

## Completion-phase systems (pre-implementation baseline)

| System | Status |
|--------|--------|
| Application Kernel | complete_and_verified |
| Application Manifest | complete_and_verified |
| Structure / state / user-data separation | partial (surface_state vs definition only; no state read path for inline UI) |
| Generated data models | complete_and_verified |
| Generated data migrations | complete_but_unverified |
| Change Compiler | complete_and_verified |
| Change-impact analysis | complete_and_verified |
| Operation provenance | complete_and_verified |
| Risk-based proposals | complete_but_unverified |
| Application permissions | complete_and_verified |
| Policy evaluation hook | complete_and_verified |
| Declarative generated tests | complete_but_unverified |
| Visual / a11y verification | complete_but_unverified |
| Agent repair loop | missing |
| Last-known-good | complete_and_verified |
| Recovery Mode | complete_but_unverified |
| Multiwindow sync | complete_but_unverified |
| Dependency graph / GC | partial |
| Responsive primitives | partial (CSS flex only) |
| Token-efficient app context | partial |
| Unified search for apps | complete_but_unverified |
| Command palette | complete_but_unverified |
| `.coreside-app` packages | complete_and_verified |
| Offline / resumable jobs | complete_but_unverified |
| Enterprise connector docs | missing |
| Enterprise policy hook | missing |

## Corrective priorities for this phase

1. Application Kernel as sole trusted mutation gateway  
2. Manifests + generated data + permissions  
3. Change Compiler + impact + provenance + proposals  
4. Testing / last-known-good / Recovery Mode  
5. Packages, offline jobs, policy hook, docs  

Machine-readable mirror: `reports/runtime-v2-completion-audit.json`
