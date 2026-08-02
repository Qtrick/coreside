# Application Viewport Contract

**Product:** Coreside  
**Status:** Normative contract for the adaptive layout phase (implementation in progress)  
**Last updated:** 2026-08-01  
**Related research:** [ADAPTIVE_LAYOUT_RESEARCH.md](./ADAPTIVE_LAYOUT_RESEARCH.md)

This contract defines how Coreside’s protected UI and generated surfaces must behave inside the native window. It is a requirement for implementation and verification — not a claim that every clause is already satisfied.

## Contract clauses

1. **Client-area authority** — The native Coreside window’s client area is the authoritative visual boundary.

2. **Root width bound** — The application root may not become wider than that client area.

3. **Navigation in bounds** — Core navigation controls may not be positioned outside that client area.

4. **No app-level horizontal text scroll** — Textual content may not require application-level horizontal scrolling.

5. **No app-level horizontal action scroll** — Buttons and actions may not require application-level horizontal scrolling.

6. **Panel bounding** — Every core panel must have:
   - `min-width: 0`
   - `min-height: 0`
   - bounded width
   - bounded height
   - an explicit scrolling owner

7. **Vertical scroll ownership** — Vertical scrolling must occur only in the panel or region intended to scroll.

8. **Horizontal overflow prevention** — Horizontal overflow must be prevented by responsive layout, wrapping, stacking, truncation with accessible disclosure, or controlled component adaptation.

9. **Root `overflow-x: hidden` last resort** — `overflow-x: hidden` may be used as a final safety guard at the root only after actual overflow causes are fixed.

10. **No concealment by clipping** — Root clipping must not be used to conceal inaccessible controls.

11. **Generated applications** — Generated applications must obey the same contract.

12. **Overlays stay inside** — Modals, menus, popovers, tooltips, and context menus must reposition to remain inside the client area.

13. **Secondary windows** — Secondary windows must obey the same contract independently.

14. **Zoom and text scaling** — Zoom and text scaling must not make core controls inaccessible.

15. **Bounded 2D exceptions** — An exception for genuinely two-dimensional content may exist only inside a clearly bounded specialized component, such as:
    - A code editor
    - A wide data visualization
    - A Canvas
    - A timeline that is inherently two-dimensional

16. **Exception limits** — Even in those exceptions:
    - Core actions remain outside the horizontal scroller
    - Text instructions remain readable without horizontal scrolling
    - Keyboard access remains possible
    - The application root never horizontally scrolls

## Scroll owners (expected)

| Region | Vertical scroll owner |
| --- | --- |
| Sidebar | Sidebar list region |
| Chat | Message list (composer sticky / non-scrolling) |
| Tool Canvas | Tool content body (header sticky where useful) |
| Settings / Projects / Media / Automations | Page content region |
| Generated surfaces | Surface root or declared internal region |
| Secondary tool window | That window’s content root |

Exact selectors are implementation details; ownership must remain unambiguous.

## Layout modes (summary)

| Mode | Expectation |
| --- | --- |
| Wide | Sidebar + chat + tool side-by-side when safe minima met |
| Standard | Condensed header / columns; still side-by-side only when minima met |
| Compact | One primary content pane; no clipped half-visible Tool Canvas |
| Full-page | Settings/Projects/Media/Automations use remaining area after sidebar; no empty third column |

## Compliance checks

- Automated: `reports/layout-overflow-baseline.json` / result reports; nested panels, not root-only.
- Manual: minimum window (900×600), display scaling, reduced motion, secondary window, zoom.
- Generated tools: inherit tokens + trusted viewport metadata; defaults when metadata absent.

## Non-goals

- Unrestricted CSS from the agent
- Application-level horizontal scrollbars as a design feature
- Treating OS minimum window size as permission to overflow
- Disabling this contract from Added Settings or agent proposals

## Protected identifiers (planned)

`core.viewport_contract` · `core.layout_modes`
