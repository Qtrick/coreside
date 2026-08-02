# Adaptive Layout Research

**Product:** Coreside  
**Phase:** Adaptive viewport / wallpaper / consumer polish  
**Status:** Research for implementation (features described here are decisions, not shipped claims)  
**Access date:** 2026-08-01

## Purpose

Define how Coreside’s protected AppShell, chat, Tool Canvas, settings pages, and generated surfaces reflow inside the native window client area with zero application-level horizontal overflow.

## Sources reviewed

| Source | URL | Access date |
| --- | --- | --- |
| Tauri v2 Window API | https://v2.tauri.app/reference/javascript/api/namespacewindow/ | 2026-08-01 |
| Tauri v2 Webview API | https://v2.tauri.app/reference/javascript/api/namespacewebview/ | 2026-08-01 |
| Tauri security / capabilities | https://v2.tauri.app/security/ | 2026-08-01 |
| Tauri configuration | https://v2.tauri.app/reference/config/ | 2026-08-01 |
| WCAG 2.2 | https://www.w3.org/TR/WCAG22/ | 2026-08-01 |
| WCAG Reflow (Understanding 1.4.10) | https://www.w3.org/WAI/WCAG21/Understanding/reflow.html | 2026-08-01 |
| WCAG Technique C31 (Reflow) | https://www.w3.org/WAI/WCAG22/Techniques/css/C31 | 2026-08-01 |
| WCAG Technique C32 (Media queries / fluid) | https://www.w3.org/WAI/WCAG22/Techniques/css/C32 | 2026-08-01 |
| MDN ResizeObserver | https://developer.mozilla.org/en-US/docs/Web/API/ResizeObserver | 2026-08-01 |
| MDN Container queries | https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Container_queries | 2026-08-01 |
| MDN CSS Containment | https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment | 2026-08-01 |
| MDN prefers-reduced-motion | https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion | 2026-08-01 |
| Vendo product + repo (lesson only) | https://vendo.run/ · https://github.com/runvendo/vendo | 2026-08-01 |
| Local Vendo audit | [VENDO_REFERENCE_AND_ADOPTION_AUDIT.md](./VENDO_REFERENCE_AND_ADOPTION_AUDIT.md) | 2026-08-01 |
| Formal contract | [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md) | — |

## Current repository baseline (inspected, not claimed complete)

- AppShell uses CSS grid in `src/styles/global.css` with sidebar / chat / tool columns.
- Main window (`tauri.conf.json`): initial ≈1280×800, `minWidth` 900, `minHeight` 600, resizable.
- Secondary tool windows open at fixed ≈720×640 (`src-tauri/src/windows/mod.rs`).
- No `ResizeObserver` and no `@container` / `container-type` usage found for layout modes.
- Tool Canvas header actions (Details, Customize, Export, Open, Undo, Close) can force min-content width past the allocated track (screenshot evidence).
- Generated surfaces inherit tokens partially; they do not yet declare trusted viewport metadata.

## Relevant findings

### WCAG reflow (C31 / C32)

- Content must reflow at ~320 CSS px width (or equivalent zoom) without requiring two-dimensional scrolling for ordinary reading and operation (exceptions: maps, data tables, diagrams, video, games, and similar 2D content).
- Prefer fluid layouts, wrapping, stacking, and container-relative sizing over fixed pixel columns.
- Application-level horizontal scroll for core chrome is a contract violation even if the OS window is larger than 320 px.

### Container queries vs viewport media queries

- Viewport breakpoints alone are insufficient: Tool Canvas at half of 1440 px is not a “1440 layout.”
- Named containers (main area, chat, tool canvas, tool header, settings content, generated surface root) should drive column counts, header compaction, and card stacking.
- Provide tested fallbacks when the WebView lacks container-query support (class toggles from a single ResizeObserver-derived layout mode).

### ResizeObserver

Use only for behavioral coordination, not for every CSS decision:

- Measuring Tool Canvas content area vs trusted `minUsefulWidth` / `minUsefulHeight`.
- Header action fit / overflow-menu decisions when CSS alone is insufficient.
- Overflow diagnostics.
- Scheduling smart native-window expansion (see [WINDOW_ORCHESTRATION_RESEARCH.md](./WINDOW_ORCHESTRATION_RESEARCH.md)).

Requirements: one observer per meaningful container, cleanup on unmount, debounce, no resize feedback loops with native animation.

### Reduced motion

- Layout mode changes and split-ratio updates must remain usable with `prefers-reduced-motion: reduce`.
- Prefer near-instant final geometry over decorative transitions.
- Live wallpapers must respect reduced motion (pause / static frame) independently of layout.

## Decisions

| Decision | Choice |
| --- | --- |
| Layout authority | Native window client area; see Viewport Contract |
| Mode model | Wide / Standard / Compact (+ full-page overlays) |
| Primary techniques | CSS Grid + Flexbox + container queries; ResizeObserver only when needed |
| Compact mode | One primary content pane; never a clipped half-visible Tool Canvas |
| Split | Resizable chat/tool divider with persisted ratio; clamp on shrink; compact when minima unmet |
| Overflow policy | Fix causes first; `overflow-x: hidden` only as root last-resort guard |
| Generated tools | Trusted viewport metadata + semantic tokens; no arbitrary CSS |
| Vendo lesson | Capabilities stay inside product chrome, inherit visual language, obey host guardrails, fit available surface |

## Rejected approaches

| Approach | Why rejected |
| --- | --- |
| Global `overflow-x: hidden` without fixing min-content offenders | Hides inaccessible Close / actions |
| Viewport-only breakpoints for Tool Canvas | Ignores allocated column width |
| JS-driven pixel layout for every panel | Fragile; CSS should own reflow |
| Unrestricted generated CSS / iframes | Violates protected-core generative UI security |
| Copying Vendo source or branding | License/provenance + product identity; lesson-only adoption |
| Treating `minWidth: 900` as permission to overflow | Minimum size still requires coherent layout |

## Security implications

- Layout mode and split ratio are UI/continuity state; they must not grant generated surfaces window or filesystem APIs.
- Overflow menus and portals remain in the protected React shell; generated content cannot position OS windows.
- Agent-proposed viewport metadata is clamped by trusted limits; agent cannot disable compact mode or Recovery Mode.

## Performance implications

- Container queries and a small number of ResizeObservers are cheap vs continuous measuring every child.
- Avoid layout thrash during native window animation (coalesce measurements after animation settle).
- Live wallpaper canvas resize must batch DPR / size updates (see wallpaper research).

## Rollback

1. Feature-flag adaptive layout modes; fall back to current fixed grid.
2. Keep Viewport Contract tests optional until green.
3. Revert container-query CSS independently of wallpaper / window orchestration flags.
4. Persisted split ratio: ignore unknown keys; default 50/50 within safe minima.

## Related docs

- [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md)
- [WINDOW_ORCHESTRATION_RESEARCH.md](./WINDOW_ORCHESTRATION_RESEARCH.md)
- [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md)
- [CONSUMER_POLISH_RESEARCH.md](./CONSUMER_POLISH_RESEARCH.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
