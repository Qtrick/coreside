# Consumer Polish Audit

**Product:** Coreside  
**Date:** 2026-08-01  
**Status:** Initial audit from screenshot evidence + repository inspection  
**Machine-readable:** `reports/consumer-polish-findings.json`  
**Research:** [CONSUMER_POLISH_RESEARCH.md](./CONSUMER_POLISH_RESEARCH.md)

This audit classifies consumer-visible polish and layout defects for the adaptive viewport / wallpaper phase. It does **not** claim fixes are complete. Preserve Coreside visual identity; do not treat this as a redesign brief.

## Severity legend

| Level | Meaning |
| --- | --- |
| P0 | Blocks trust or core usability; fix in this phase |
| P1 | Major polish / readability; fix in this phase |
| P2 | Related layout/overflow/readability; fix when adjacent |
| P3 | Nice-to-have; track only |

## P0 findings

### POLISH-P0-001 — Wallpaper None writes invalid schema JSON

- **Evidence:** Screenshot / Settings error: `wallpaper JSON invalid: missing field schemaVersion`.
- **Cause (architectural):** Legacy `{ kind: "none" }` (or equivalent) routed into schema-only `wallpaperJson`, which Rust validates for `schemaVersion` + `type`.
- **Required fix:** Trusted clear-wallpaper path; never persist legacy none into schema field; friendly consumer error if validation still fails.
- **Refs:** [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md), [WALLPAPERS.md](./WALLPAPERS.md)

### POLISH-P0-002 — Tool Canvas header overflows client area

- **Evidence:** Screenshot of Tool Canvas actions (Details, Customize, Export, Open, Undo, Close) extending past the window edge; title/description collapsing poorly.
- **Cause:** Header/action row lacks constrained-width negotiation (`min-width: 0`, wrap/stack/overflow menu); grid min-content inflated.
- **Required fix:** Responsive header per Adaptive Layout + Viewport Contract; Close/Back always reachable.
- **Refs:** [ADAPTIVE_LAYOUT_RESEARCH.md](./ADAPTIVE_LAYOUT_RESEARCH.md), [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md)

### POLISH-P0-003 — Gradient-clipped Coreside wordmark

- **Evidence:** `.wordmark-text` uses linear gradient + `background-clip: text` + transparent fill (`src/styles/global.css`).
- **Cause:** Decorative gradient text on protected brand wordmark.
- **Required fix:** Solid `--text-primary` / `--core-text-primary` for wordmark text in light and dark themes.
- **Refs:** [CONSUMER_POLISH_RESEARCH.md](./CONSUMER_POLISH_RESEARCH.md), [BRANDING.md](./BRANDING.md)

## P1 findings

### POLISH-P1-001 — Wallpaper occluded by near-opaque panels

- **Evidence:** Live wallpaper barely visible behind chat/tool/sidebar; overlay tokens ≈86–92% surface mix; additional panel `color-mix` layers.
- **Cause:** No separate interface-transparency setting; hardcoded high alphas; possible body gradient competition.
- **Required fix:** Interface transparency (0–60%, default 20%) + semantic alpha tokens + readability enforcement; layering audit.
- **Refs:** [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md), [READABILITY.md](./READABILITY.md)

## P2 findings (initial)

| ID | Title | Notes |
| --- | --- | --- |
| POLISH-P2-001 | Inconsistent panel header padding/action alignment | Align during Tool header work |
| POLISH-P2-002 | Nested / ambiguous scroll regions | Document scroll owners; fix offenders found by overflow audit |
| POLISH-P2-003 | Menus/popovers not guaranteed viewport-safe | Window-aware repositioning |
| POLISH-P2-004 | Reduced-motion incomplete for layout/wallpaper | Extend existing partial support |
| POLISH-P2-005 | Secondary tool window fixed 720×640 | Metadata-aware sizing later in window orchestration |
| POLISH-P2-006 | Raw technical errors in other settings paths | Sanitize when touched |

## P3 (tracked)

| ID | Title |
| --- | --- |
| POLISH-P3-001 | Empty/loading/failure state consistency across all pages |
| POLISH-P3-002 | Icon-only density polish beyond Tool header |
| POLISH-P3-003 | Separate blur control for wallpapers |

## Out of scope for this audit

- Enterprise org admin / SSO
- Unrestricted HTML/JS generative surfaces
- Full WCAG certification claim
- Unrelated subsystem redesigns

## Verification plan

1. Reproduce each P0 with packaged or `npm run dev` desktop build.  
2. Run overflow audit → `reports/layout-overflow-*.json`.  
3. Wallpaper None → restart persistence check.  
4. Light/dark wordmark contrast check.  
5. Transparency slider + readability protection on live presets.  
6. Update `reports/consumer-polish-findings.json` `status` fields when fixed.

## Related

- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
- [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)
