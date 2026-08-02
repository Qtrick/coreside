# Wallpaper Compositing Graph

**Product:** Coreside  
**Access date:** 2026-08-02  
**Report:** `reports/wallpaper-compositing-graph.json`

## Layer order (bottom → top)

| # | Layer | Owner | Background | Exposes wallpaper? |
| --- | --- | --- | --- | --- |
| 1 | HTML / body fallback | `global.css` | solid `--surface` / theme | No (fallback only) |
| 2 | Root app shell | `AppShell` | transparent / content overlay | Yes when wallpaper active |
| 3 | Live wallpaper renderer | `LiveWallpaper` | Canvas / media | Wallpaper base |
| 4 | Readability scrim | transparency tokens | `--core-readable-scrim-alpha` | Softens contrast |
| 5 | Sidebar | shell | `--core-sidebar-overlay` | Yes (panel depth) |
| 6 | Main content / chat / tools | panels | `--core-content-overlay` | Yes |
| 7 | Settings sections / cards | section / card | `--core-*-overlay` | Yes with higher min alpha |
| 8 | Controls / inputs | control | `--core-control-overlay` | Limited (readability) |
| 9 | Menus / elevated | elevated | near-opaque overlays | Intentional exception |
| 10 | Modals / recovery / approvals | modal / critical | `--core-modal-overlay` / solid | Intentional opaque |

## Semantic depths

- **wallpaper-base** — renderer only  
- **shell / panel / section / card / control / modal / critical** — alpha from `computeInterfaceTransparencyTokens`  
- Exceptions documented in `reports/background-rule-inventory.json`

## Proof status

| Proof | Status |
| --- | --- |
| Token / CSS variable monotonicity | Passed (`test:wallpaper-compositing`) |
| Synthetic panel blend contribution | Passed (`wallpaper-visual-proof.ts`) |
| Desktop pixel screenshots 0/20/40/60 | **Not done** |
| Packaged WebView proof | **Not done** |
| Light + dark desktop | **Not done** |

## Root cause (historical)

Nested opaque `var(--surface)` fills under translucent panels blocked wallpaper visibility. High-traffic surfaces now use `--core-*-overlay` tokens.
