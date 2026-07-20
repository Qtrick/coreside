# Accessibility Assurance

**Product:** Coreside v0.1.0  
**Last updated:** 2026-07-19

## Implemented

| Area | Status | Evidence |
| --- | --- | --- |
| Semantic color tokens | **Implemented** | `src/styles/tokens.css`, `docs/READABILITY.md` |
| Wallpaper readability overlays | **Implemented** | `--core-content-overlay`, contrast helpers |
| WCAG contrast math | **Implemented** | `src/lib/readability/contrast.ts` — 10 unit tests |
| Focus-visible styles | **Partial** | Present on many controls; not audited app-wide |
| Reduced motion | **Partial** | Some CSS respects `prefers-reduced-motion`; not comprehensive |
| Settings → Accessibility section | **Implemented** | Base Settings structure in protected core |

## Pending / not verified

| Area | Status | Notes |
| --- | --- | --- |
| Full keyboard navigation | **Pending** | No automated a11y test suite (axe, etc.) |
| Chat composer focus trap | **Pending** | Manual test on Tauri build |
| Sidebar / project list roving tabindex | **Pending** | Not audited |
| Secondary window focus order | **Pending** | Journey K manual |
| Screen reader labels on inline surfaces | **Partial** | Scenario H checks unlabeled control warning — manual |
| Command palette (Cmd+K) a11y | **Pending** | Scenario W manual |
| Narrow layout (~360px) | **Pending** | Scenario Y manual |
| Light/dark contrast audit | **Pending** | Scenario X manual; unit tests cover math only |
| WCAG 2.2 formal conformance claim | **Not claimed** | No audit certificate |

## Automated coverage today

```bash
npm run test -- src/lib/readability/contrast.test.ts
```

Vitest does **not** run `@axe-core` or Playwright accessibility scans.

## Manual checklist hooks

From `docs/MANUAL_ACCEPTANCE_A_AB.md`:

- **H** — Inline surface a11y warnings
- **U** — Wallpaper readability with live background
- **W** — Command palette keyboard
- **X** — Theme readability
- **Y** — Narrow layout usability

## Recommendations (future)

1. Add axe-core to Vitest for critical settings/chat components.
2. Document tab order for main shell (sidebar → chat → composer).
3. WebDriver: keyboard-only path for A, C, W on packaged build.

## Honest assessment

Coreside has **foundational** readability and contrast tooling. It does **not** yet have evidence of full keyboard/screen-reader assurance across all surfaces.
