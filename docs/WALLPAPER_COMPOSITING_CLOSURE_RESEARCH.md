# Wallpaper Compositing Closure Research

**Product:** Coreside  
**Access date:** 2026-08-02 (evening)  
**Status:** **IN SCOPE** for RC; public beta **NOT READY**  
**Baseline:** archive `b2f8bf4e…` · commit `90b99538…`  
**Reports:** `reports/wallpaper-compositing.json` · `reports/background-rule-inventory.json`

## Verdict

Nested opaque `--surface` / tint-onto-surface occlusion is largely migrated to overlay tokens. **Visual proof at 0/20/40/60% and None remains the closure gate.** Schema None P0 and interface-transparency setting are treated as fixed/implemented per current report.

## Root cause (unchanged)

Translucent panels were covered by nested opaque `var(--surface)` fills. Raising Interface transparency alone cannot show wallpaper until nested chrome uses `--core-*-overlay` tokens.

## Closure checklist

| Item | Status |
| --- | --- |
| Overlay token emission | Done (per report) |
| High-traffic nested migrations | Advanced |
| Remaining surface risk | Low / intentional exceptions |
| Visual proof pack (0/20/40/60% + None) | **Not done** — RC exit |
| Readability clamps / scrims | Must remain non-disableable |
| Unrelated wallpaper redesign | Out of scope |

## None behavior (must hold)

- Clear schema `wallpaperJson`; never persist legacy `{kind:"none"}` as schema.
- Solid surfaces when inactive; transparency preference may persist.
- Friendly consumer error on invalid schema; no wipe of unrelated settings.

## Related

- [WALLPAPER_ASSURANCE.md](./WALLPAPER_ASSURANCE.md)
- [SURFACE_COMPOSITING_MODEL.md](./SURFACE_COMPOSITING_MODEL.md)
- [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md)
- [RELEASE_CANDIDATE_RESEARCH.md](./RELEASE_CANDIDATE_RESEARCH.md)
