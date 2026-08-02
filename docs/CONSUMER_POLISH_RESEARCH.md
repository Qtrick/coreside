# Consumer Polish Research

**Product:** Coreside  
**Phase:** Adaptive viewport / wallpaper / consumer polish  
**Status:** Research for implementation (not shipped claims)  
**Access date:** 2026-08-01

## Purpose

Define restrained consumer-facing polish for layout, chrome, errors, motion, and native-feeling generated tools — without redesigning Coreside’s visual identity.

## Sources reviewed

| Source | URL / path | Access date |
| --- | --- | --- |
| WCAG 2.2 | https://www.w3.org/TR/WCAG22/ | 2026-08-01 |
| WCAG Reflow / C31 / C32 | Understanding reflow · C31 · C32 | 2026-08-01 |
| MDN prefers-reduced-motion | https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion | 2026-08-01 |
| MDN Container queries / ResizeObserver | MDN containment + ResizeObserver | 2026-08-01 |
| Tauri window / security (chrome context) | https://v2.tauri.app/reference/javascript/api/namespacewindow/ · https://v2.tauri.app/security/ | 2026-08-01 |
| Branding | [BRANDING.md](./BRANDING.md) | 2026-08-01 |
| Vendo adoption audit | [VENDO_REFERENCE_AND_ADOPTION_AUDIT.md](./VENDO_REFERENCE_AND_ADOPTION_AUDIT.md) | 2026-08-01 |
| Audit ledger | [CONSUMER_POLISH_AUDIT.md](./CONSUMER_POLISH_AUDIT.md) | — |

## Vendo product lessons (adopt concepts, not code)

From the local audit and Vendo’s product posture:

1. User-created capabilities remain **inside** the product surface.
2. They inherit the product’s **visual language** (tokens, spacing, typography, states).
3. They obey **host-defined permissions** and guardrails.
4. They **fit** the available product surface (responsive contract).
5. They must not look like disconnected, unstyled mini-websites.

Do not copy Vendo source without license/provenance review. Do not add Vendo branding. Coreside remains the product name and identity.

## Screenshot-driven problem themes (2026-08-01)

| Theme | Severity | Symptom |
| --- | --- | --- |
| Wallpaper None → schema error | P0 | Raw `missing field schemaVersion` |
| Tool header overflow | P0 | Actions / Close past client edge |
| Gradient wordmark | P0 | `background-clip: text` + transparent fill on `.wordmark-text` |
| Wallpaper occlusion | P1 | Panels ~88–92% opaque; wallpaper barely visible |

Full classification: [CONSUMER_POLISH_AUDIT.md](./CONSUMER_POLISH_AUDIT.md) · `reports/consumer-polish-findings.json`.

## Decisions

### Solid text policy

Protected primary text and the Coreside wordmark use **solid** theme tokens (`--core-text-primary` / `--text-primary`).  
Remove gradient-clipped text, transparent fills, and accent gradients on protected headings.  
Logo mark assets may keep branded mark treatment; wordmark **text** stays solid.

### Consumer-friendly errors

Replace raw validator text with copy such as:  
“Coreside could not apply that wallpaper. The previous appearance was restored.”  
Developer Mode may show redacted technical detail.

### Action hierarchy

Tool header priority (validate in UX):

- Always: Close/Back (+ one primary contextual action when present)
- When space: Customize, Open, Undo
- Overflow-eligible: Details, Export, future secondary actions

No horizontally scrolling toolbar. Title/description get `min-width: 0`. Menus stay viewport-safe.

### Motion

- Ordinary transitions ≈160–240 ms; window expansion ≈180–260 ms.
- `prefers-reduced-motion: reduce` → near-instant final state.
- No decorative motion that delays interaction; pause hidden wallpaper work.

### Scroll ownership

One obvious vertical scroll owner per major panel. Sticky composer reachable. Nested scroll only with strong reason. See Viewport Contract.

### Selection vs preview

Active wallpaper, chat, project, and tool selection must match persisted state. Preview ≠ applied.

### Generated surfaces

Inherit Coreside tokens, spacing, typography, interaction states, permissions, and responsive behavior. No arbitrary embedded webpage aesthetic.

### No placeholder alerts

`window.alert` and “coming later” controls in core flows must be functional, clearly disabled with explanation, or removed.

## Rejected approaches

| Approach | Why rejected |
| --- | --- |
| Full visual redesign / purple AI gradients | Violates Coreside design language |
| Cards everywhere / dashboard clutter | Consumer calm UI |
| Emoji navigation | Brand/UX mismatch |
| Fixing overflow only with root clip | Hides unreachable controls |
| Shipping raw Rust/Zod errors to consumers | Trust and polish failure |
| Transplanting Vendo UI | Wrong product identity |

## Security implications

- Polish changes stay in protected shell CSS/components; no new generative execution surface.
- Error sanitization must not hide Action Log / Recovery when those are enabled and appropriate.
- Menu repositioning must not escape capability boundaries.

## Performance implications

- Overflow menus and container queries are cheaper than perpetual measuring.
- Prefer CSS for condensation; ResizeObserver only for fit decisions that CSS cannot express.
- Avoid animating large blurred layers.

## Rollback

1. Revert wordmark / token CSS independently.
2. Keep wallpaper clear-path fix even if transparency UI rolls back.
3. Feature-flag overflow-menu header; fall back to wrapping row only if it still meets Viewport Contract.
4. Audit JSON remains historical evidence; do not delete findings on rollback.

## Related docs

- [CONSUMER_POLISH_AUDIT.md](./CONSUMER_POLISH_AUDIT.md)
- [ADAPTIVE_LAYOUT_RESEARCH.md](./ADAPTIVE_LAYOUT_RESEARCH.md)
- [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md)
- [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
