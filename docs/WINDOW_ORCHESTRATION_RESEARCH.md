# Window Orchestration Research

**Product:** Coreside  
**Phase:** Adaptive viewport / wallpaper / consumer polish  
**Status:** Research for implementation (not shipped claims)  
**Access date:** 2026-08-01

## Purpose

Define a trusted native-window coordinator so Coreside may expand, retract, and size windows only when CSS reflow cannot deliver a usable tool experience — without giving generated content or the agent raw window control.

## Sources reviewed

| Source | URL | Access date |
| --- | --- | --- |
| Tauri v2 Window API | https://v2.tauri.app/reference/javascript/api/namespacewindow/ | 2026-08-01 |
| Tauri v2 Webview API | https://v2.tauri.app/reference/javascript/api/namespacewebview/ | 2026-08-01 |
| Tauri security / capabilities | https://v2.tauri.app/security/ | 2026-08-01 |
| Tauri configuration | https://v2.tauri.app/reference/config/ | 2026-08-01 |
| WCAG Reflow / C31 / C32 | https://www.w3.org/WAI/WCAG21/Understanding/reflow.html · C31 · C32 | 2026-08-01 |
| MDN ResizeObserver | https://developer.mozilla.org/en-US/docs/Web/API/ResizeObserver | 2026-08-01 |
| MDN prefers-reduced-motion | https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion | 2026-08-01 |
| Platform notes | macOS / Windows work areas, logical vs physical pixels, scaling, fullscreen | 2026-08-01 |
| Vendo adoption audit | [VENDO_REFERENCE_AND_ADOPTION_AUDIT.md](./VENDO_REFERENCE_AND_ADOPTION_AUDIT.md) | 2026-08-01 |

## Current repository baseline

- Main window: width 1280, height 800, `minWidth` 900, `minHeight` 600 (`src-tauri/tauri.conf.json`).
- Secondary tool window: fixed `inner_size(720, 640)` (`src-tauri/src/windows/mod.rs`); shared SQLite; capability split already rejects shell on `tool-*` windows.
- Multi-window revision conflict UX exists; no smart expansion, restoration snapshot, or Adaptive Window Sizing setting exists yet.
- Frontend must not call arbitrary `setSize` / `setPosition` from generated surfaces.

## Tauri API decisions

| Need | Approach |
| --- | --- |
| Read size / position / maximized / fullscreen | Trusted Window Orchestrator via Tauri Window APIs (`innerSize`, `outerPosition`, `isMaximized`, `isFullscreen`) |
| Set bounds | Single Rust-side coordinator; frontend requests decisions, does not stream per-frame IPC |
| Monitor work area | Public Tauri monitor APIs only; clamp expansion to current monitor |
| Events | Listen resize/move; debounce settle; classify manual vs orchestrated |
| Capabilities | Minimum permissions on **main** window only unless a separate justified tool-window path exists |
| Animation | One active bounds animation; supersession; ~180–260 ms; reduced motion → apply final bounds immediately |
| Private APIs | Rejected: no private macOS APIs, no shell helpers, no external window utilities |

### Order of operations before native expansion

1. CSS reflow  
2. Header compaction  
3. Sidebar compaction (only when consistent with existing UX)  
4. Split-ratio clamp within safe bounds  
5. Re-measure  
6. Consider native expansion if policy allows  
7. Monitor-aware clamp  
8. Animate / apply  
9. Re-measure  
10. Fall back to compact mode or “Open in new window”

## Adaptive Window Sizing setting (planned)

Protected Base Settings (Appearance / Window Behavior — not Added Settings):

| Option | Behavior |
| --- | --- |
| **Smart** (default) | May expand within current monitor when min useful size unmet |
| **Ask first** | Compact prompt; tool remains usable while waiting |
| **Off** | Never auto-resize; reflow / compact / secondary window |

Runtime agent may not change this preference. Persist separately from user-preferred bounds and auto-expanded bounds.

## Expansion direction

Trusted directions: `right` | `left` | `down` | `up` | `balanced` | `automatic`.  
Default for Tool Canvas opening beside chat: `right` (keep left edge stable when possible). Never jump monitors without explicit user action. Never leave the window partially off-work-area.

## Restoration rules

- Snapshot user-preferred bounds before auto-expansion.
- Retract only if Coreside performed the expansion and the user did not manually resize afterward.
- Do not store auto-expanded bounds as preferred.
- Do not shrink while another tool still needs space, during drag, or while maximized/fullscreen.
- Manual resize after expansion cancels automatic restoration and becomes authoritative.

## Maximized / fullscreen

- No automatic `setSize` / `setPosition`.
- Reflow inside client area; compact only when required.
- Do not unexpectedly unmaximize or exit fullscreen.

## Secondary tool windows

- Size from trusted tool viewport metadata + safe defaults.
- Clamp to current monitor work area; open fully onscreen.
- Remember user-resized bounds per tool/class where appropriate.
- Preserve theme, continuity, and protected capabilities (no shell).
- Do not auto-open every tool in a secondary window.

## Typed models (planned)

`WindowBounds`, `LogicalWindowBounds`, `MonitorWorkArea`, `WindowLayoutMode`, `WindowResizeSource`, `ToolViewportPolicy`, `WindowExpansionRequest`, `WindowExpansionDecision`, `WindowExpansionSession`, `WindowRestorationSnapshot`, `WindowAnimationState`, `ManualResizeState`.

Do not expose raw monitor dumps to the model.

## Protected resource identifiers (planned)

- `core.window_orchestrator`
- `core.window_bounds_policy`
- `core.adaptive_window_sizing`
- `core.window_animation`
- `core.monitor_bounds`
- `core.viewport_contract`
- `core.layout_modes`

## Rejected approaches

| Approach | Why rejected |
| --- | --- |
| Frontend per-frame `setSize` during animation | IPC spam; jank; feedback loops |
| Expanding to “ideal” size by default | Ideal ≠ minimum useful; aggressive and surprising |
| Letting agent/tools call window APIs | Security boundary violation |
| CRDT / multi-user window sync | Out of consumer scope |
| Private OS animation APIs | Unsupported / fragile / review risk |
| Persisting temporary expansion as user preference | Corrupts restore behavior |

## Security implications

- Window sizing is a protected-core capability; generated surfaces never receive it.
- Capability grants must stay minimal and window-scoped.
- Orchestrator validates clamps, minima, maxima, and policy before any bounds change.
- Adversarial tests: forged expansion requests, oversized metadata, rapid supersession.

## Performance implications

- Single animation owner; frame coalescing; no main-thread blocking Rust loops.
- Debounce resize-end before persistence and re-evaluation.
- Cooldown after manual resize before reconsidering auto-expansion.
- Avoid ResizeObserver ↔ animation feedback.

## Platform limitations (honest)

- Logical vs physical pixels differ by scale factor; always convert consistently.
- Work-area APIs and animation smoothness differ across macOS and Windows; document adapter behavior per OS.
- Multi-monitor: identify current monitor; on missing prior monitor, clamp onto an available work area.
- Exact public animated-bounds APIs may be absent; fall back to trusted bounded interpolation.

## Rollback

1. Feature-flag Adaptive Window Sizing; treat missing setting as **Off**.
2. Disable Rust animation path; leave CSS reflow / compact mode active.
3. Secondary windows fall back to fixed 720×640 defaults.
4. Clear in-flight expansion sessions; do not rewrite preferred bounds on rollback.

## Related docs

- [ADAPTIVE_LAYOUT_RESEARCH.md](./ADAPTIVE_LAYOUT_RESEARCH.md)
- [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md)
- [MULTIWINDOW_CONCURRENCY.md](./MULTIWINDOW_CONCURRENCY.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
