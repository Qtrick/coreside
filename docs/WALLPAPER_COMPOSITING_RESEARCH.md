# Wallpaper Compositing Research

**Product:** Coreside  
**Phase:** Adaptive viewport / wallpaper / consumer polish  
**Status:** Research for implementation (not shipped claims)  
**Access date:** 2026-08-01

## Purpose

Separate wallpaper intensity from interface transparency, keep the wallpaper as the true base layer, preserve readability, and fix ambiguous “None” / schema persistence paths.

## Sources reviewed

| Source | URL / path | Access date |
| --- | --- | --- |
| WCAG 2.2 contrast / non-text | https://www.w3.org/TR/WCAG22/ | 2026-08-01 |
| MDN prefers-reduced-motion | https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion | 2026-08-01 |
| MDN ResizeObserver | https://developer.mozilla.org/en-US/docs/Web/API/ResizeObserver | 2026-08-01 |
| CSS Color / contrast practice | MDN + existing `src/lib/readability/contrast.ts` | 2026-08-01 |
| Existing wallpaper docs | [WALLPAPERS.md](./WALLPAPERS.md), [LIVE_WALLPAPERS.md](./LIVE_WALLPAPERS.md), [READABILITY.md](./READABILITY.md) | 2026-08-01 |
| Vendo lesson | [VENDO_REFERENCE_AND_ADOPTION_AUDIT.md](./VENDO_REFERENCE_AND_ADOPTION_AUDIT.md) — host visual language, not arbitrary mini-sites | 2026-08-01 |

## Current repository baseline

- Schema wallpapers: `schemaVersion: "1"` + `type` validated in Rust (`src-tauri/src/wallpapers/`).
- Frontend resolver: `resolveActiveWallpaper` / `parseWallpaperJson` (`src/lib/wallpaper.ts`).
- Live renderer: `LiveWallpaper` under AppShell; `data-wallpaper` drives overlay tokens.
- Overlay tokens today (`tokens.css`): content ≈90–92% surface mix; sidebar ≈86–88% — wallpaper barely shows through (screenshot occlusion).
- Sidebar / chat also use additional `color-mix(... 88–92%)` in `global.css`, compounding opacity.
- Body/root decorative gradients can compete with or obscure live wallpapers if not audited.
- Known P0 path: posting legacy `{ kind: "none" }` into schema `wallpaperJson` produces `missing field schemaVersion` (consumer-visible raw error).

## Two distinct concepts

| Concept | Meaning | Control |
| --- | --- | --- |
| **Wallpaper intensity** | How strongly the wallpaper renderer draws canvas / media / gradient | Wallpaper config / preset opacity where applicable |
| **Interface transparency** | How translucent protected panels are over the wallpaper | New user setting (planned) |

Do **not** use one ambiguous opacity for both.  
Do **not** apply CSS `opacity` on parent panels (fades text, icons, focus). Use alpha in background colors / semantic tokens.

## Interface transparency setting (planned)

Location: **Added Settings → Templates → Wallpapers**  
Label: Interface transparency  
Range: **0%–60%** (step 1%); default **20%**  
Presets: Solid 0% · Balanced 20% · Immersive 40%  
Max allowed: 60% (not 100%).

- Persist even when wallpaper is `None`; no visual effect until a wallpaper is active.
- Agent may propose changes only via protected setting-change approval; may not exceed max, disable readability, or hide Recovery / warnings / focus.

## Token architecture (planned)

Derive from one tested computation (names may align with existing `--core-*` system):

- `--interface-transparency`
- `--core-panel-alpha` / `--core-sidebar-alpha`
- `--core-card-alpha` / `--core-control-alpha` / `--core-header-alpha`
- `--core-modal-alpha` / `--core-readable-scrim-alpha`

Principles: panels follow user setting; cards/controls/modals retain stronger minimums; composer stays distinct; generated tools inherit tokens.

## Layering order

1. Neutral root fallback  
2. Active wallpaper renderer  
3. Optional wallpaper readability treatment  
4. Translucent top-level panels  
5. Cards and controls  
6. Menus and modals  
7. Focus / a11y indicators  

When no wallpaper: solid theme background; transparency must not reveal decorative body gradients as a fake wallpaper.  
When wallpaper active: no opaque root covering it; no accidental double-darkening; canvas/media fill client area without expanding document size.

## Backdrop filters

- Optional restrained blur on translucent surfaces only.
- Not the sole readability mechanism; feature-detect + non-blur fallback.
- Avoid nested high-cost blur on every card; do not animate blur continuously.
- No separate user blur control required this phase unless evidence demands it.

## Readability enforcement

Transparency never overrides readability. Protected behavior may raise local effective opacity, add scrims, strengthen borders, or reduce wallpaper intensity behind critical surfaces — **without** silently rewriting the saved transparency preference. Optional subtle note: “Readability protection is active for this wallpaper.”

## Canonical wallpaper representation (planned)

| State | Persisted form |
| --- | --- |
| No wallpaper | `wallpaperJson` null/empty per settings conventions; legacy `wallpaper.kind` = `none` |
| Schema wallpaper | Requires `schemaVersion` + `type` + valid fields |
| Legacy | Read for compatibility; new selections prefer schema |

**Never** persist `{"kind":"none"}` into the schema-only `wallpaperJson` field.

Trusted operations: **Clear workspace wallpaper** / **Clear project wallpaper** — clear schema + reset legacy, preserve interface-transparency preference, emit cross-window events, return normalized settings.

## Canvas / live wallpaper resize

- Batch DPR and size updates; do not rebuild particles on every tiny event.
- Pause or reduce work when hidden/minimized; respect reduced motion.
- Clamp DPR per performance policy; use correct `object-fit` for media.
- Stay within client area during window animation.

## Rejected approaches

| Approach | Why rejected |
| --- | --- |
| Parent `opacity` for panels | Fades children and focus rings |
| Remote URLs as persistent wallpaper sources | Existing validation forbids; security/privacy |
| Agent-injected arbitrary CSS wallpapers | Protected renderer + schema only |
| Disabling readability to “see more wallpaper” | Non-negotiable product rule |
| Treating body gradients as wallpaper | Confuses None vs active wallpaper |
| Raw serde/Zod errors in consumer UI | Polish / trust failure |

## Security implications

- Wallpaper JSON remains Rust-validated; no script URLs, `@import`, or remote persistent sources.
- Clear/set paths are trusted commands; frontend None button must call clear, not invent schema.
- Interface transparency is bounded; agent cannot make the window invisible or hide protected chrome.

## Performance implications

- Higher transparency increases backdrop cost if blur is used — keep blur restrained.
- Live presets (matrix, particles, …) need frame caps and offscreen suspension.
- Multi-window: avoid duplicate full-rate wallpaper work where policy allows pause.

## Rollback

1. Feature-flag interface transparency; fall back to current overlay token constants.
2. Keep clear-wallpaper command; if unavailable, frontend writes null/empty + legacy none (never `{kind:"none"}` as schema JSON).
3. Invalid schema falls back to no wallpaper with friendly error; do not wipe unrelated settings.
4. Revert token computation independently of layout/window flags.

## Related docs

- [WALLPAPERS.md](./WALLPAPERS.md)
- [LIVE_WALLPAPERS.md](./LIVE_WALLPAPERS.md)
- [READABILITY.md](./READABILITY.md)
- [CONSUMER_POLISH_RESEARCH.md](./CONSUMER_POLISH_RESEARCH.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
