# Surface Compositing Model

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Normative model for protected UI; migration in progress  
**Code:** `src/lib/interface-transparency.ts`, `src/styles/tokens.css`, `src/styles/global.css`

## Goal

Let wallpapers show through protected chrome at a user-chosen transparency while keeping text, icons, focus rings, and critical controls readable. Separate **wallpaper intensity** (renderer) from **interface transparency** (panel alpha).

## Hard rules

1. **No parent `opacity`** on panels, cards, or chrome. Opacity fades children and focus indicators.
2. Translucency uses **alpha in background colors** (`color-mix` / overlay tokens), never whole-subtree fade.
3. When wallpaper is inactive (`None`), surfaces render **solid** regardless of saved transparency preference (preference still persists).
4. Readability enforcement may raise effective alpha or add scrims **without** rewriting the saved preference.
5. Agent cannot exceed the 60% max, disable readability, or hide Recovery / warnings / focus.

## Depth categories

| Category | Role | Typical token | Notes |
| --- | --- | --- | --- |
| **canvas** | Wallpaper / live renderer | (renderer opacity) | Base layer; fills client area |
| **shell** | AppShell frame, root wash | panel / solid when no wallpaper | Must not fake wallpaper via body gradient when None |
| **panel** | Chat, tools, settings, media pages | `--core-content-overlay` / `--core-panel-alpha` | Follows user transparency |
| **section** | Grouped blocks inside a panel | `--core-card-overlay` | Was a common opaque `--surface` offender |
| **card** | Bubbles, proposal cards, list cards | `--core-card-overlay` / `--core-muted-overlay` | Stronger min alpha than panel |
| **control** | Buttons, inputs, toggles | `--core-control-overlay` | Stronger than cards |
| **elevated** | Popovers, menus, command palette lists | control/modal overlays | Must remain legible over wallpaper |
| **modal** | Dialogs, command palette shell | `--core-modal-overlay` | Near-opaque floor |
| **critical** | Errors, destructive confirms, Recovery | high alpha / solid / danger surfaces | Prefer opaque |
| **tooltip** | Tooltips, transient hints | high alpha | Prefer opaque or near-opaque |

## Alpha mapping (user interface transparency)

User setting: **0% / 20% / 40% / 60%** (continuous 0–60 allowed; presets Solid / Balanced / Immersive).

| Setting | Intent | Panel behavior (approx.) |
| --- | --- | --- |
| **0%** | Solid | Panels opaque; wallpaper hidden behind chrome |
| **20%** | Balanced (default) | Panels translucent; wallpaper readable at edges |
| **40%** | Immersive | More wallpaper; cards/controls keep higher floors |
| **60%** | Maximum allowed | Strongest translucency; readability clamp may reduce *effective* value |

Computation lives in `computeInterfaceTransparencyTokens`: higher transparency → lower panel alpha; cards/controls/modals keep stronger minimums (`card ≥ ~0.82`, `control ≥ ~0.88`, `modal ≥ ~0.94` when wallpaper active).

CSS variables applied at runtime:

- `--interface-transparency`, `--interface-transparency-effective`
- `--core-panel-alpha`, `--core-sidebar-alpha`, `--core-card-alpha`, `--core-control-alpha`, `--core-header-alpha`, `--core-modal-alpha`
- `--core-*-overlay` color-mix strings derived from `var(--surface)` × alpha

## Layering order

1. Neutral root fallback  
2. Active wallpaper (canvas)  
3. Optional readability treatment / scrim  
4. Shell + translucent panels / sidebar  
5. Sections / cards  
6. Controls  
7. Elevated / modal / critical / tooltip  
8. Focus and a11y indicators (always on top of local chrome)

## Intentional opaque exceptions

Opaque `var(--surface)` (or solid fills) is **allowed** when translucency would harm affordance or media fidelity:

| Exception | Why |
| --- | --- |
| Attachment / result thumbnails | Media preview needs opaque backing |
| Toggle thumb | High-contrast movable control |
| Critical / danger surfaces | Trust and error clarity |
| Tooltip / modal floors when readability boosts | Temporary solidification |

Document remaining non-exception `var(--surface)` risk in `reports/background-rule-inventory.json`.

## Migration note (2026-08-01)

Reviewer migrated **`.settings-section`**, **`.message-bubble`**, and **`.btn-secondary`** from opaque/near-opaque fills to overlay tokens. Nested tint-onto-`--surface` rules remain the primary residual occlusion risk.

## Related

- [WALLPAPER_ASSURANCE.md](./WALLPAPER_ASSURANCE.md)
- [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md)
- [READABILITY.md](./READABILITY.md)
