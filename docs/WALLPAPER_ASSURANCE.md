# Wallpaper Assurance

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Root cause identified; partial fixes landed; visual proof still required for beta  
**Related code:** `src/lib/interface-transparency.ts`, `src/styles/tokens.css`, `src/styles/global.css`, `src/components/settings/WallpaperSettings.tsx`

## Symptom

Live or image wallpapers remain barely visible (or invisible) behind chat, settings, and tool chrome even when Interface transparency is raised above 0%.

## Root cause

**Nested opaque `var(--surface)` (and tint-onto-`--surface`) inside already-translucent panels.**

1. AppShell / panels correctly use `--core-content-overlay` / panel alpha from the transparency setting.
2. Child sections, bubbles, and controls historically painted **opaque** `background: var(--surface)` (or `color-mix(accent, var(--surface))`).
3. Those children sit *on top of* the translucent panel, so the wallpaper never reaches the eye—raising panel transparency alone cannot help.

This is distinct from the earlier P0 that wrote invalid `{kind:"none"}` schema JSON (that path is fixed via clear/normalize). Nested occlusion is a **compositing** failure, not a schema failure.

## Fix path

1. **Tokenize overlays** — `--core-card-overlay`, `--core-muted-overlay`, `--core-control-overlay`, `--core-modal-overlay` (see `applyInterfaceTransparencyCssVars`).
2. **Migrate high-traffic nested chrome** to overlays:
   - `.settings-section` → `--core-card-overlay` (**done**)
   - `.message-bubble` → `--core-card-overlay` (**done**)
   - `.btn-secondary` → `--core-control-overlay` (**done**)
3. **Inventory remaining rules** — `reports/background-rule-inventory.json`; migrate nested occlusion risks or mark intentional opaque exceptions.
4. **None path** — clear wallpaper via trusted empty/null schema + legacy none; solid surfaces when inactive; do not reveal decorative body wash as a fake wallpaper.
5. **Readability** — clamps and scrims may raise effective alpha; never disable for “more wallpaper.”

## None behavior

| Step | Expected |
| --- | --- |
| User chooses None / Clear | Schema `wallpaperJson` cleared; legacy kind `none`; no `schemaVersion` error |
| Restart | No wallpaper renderer; solid theme background |
| Transparency preference | Still stored; no visual translucency until a wallpaper is active |
| Invalid schema | Friendly consumer error; fall back to no wallpaper; do not wipe unrelated settings |

## Visual proof requirements (beta gate)

Do **not** mark wallpaper compositing complete without screenshots (or desktop capture) showing:

| Case | Must show |
| --- | --- |
| Live wallpaper + Balanced (20%) | Wallpaper visible through chat/sidebar gutters; text still readable |
| Live wallpaper + Immersive (40%) | More wallpaper; cards/controls still legible |
| Solid (0%) | Chrome opaque; wallpaper not required to show through panels |
| None | Solid theme; no body-gradient impersonating wallpaper |
| Dark + light | Same compositing behavior; wordmark remains solid `--core-text-primary` |
| Reduced motion | Live presets pause/reduce; translucency still correct |

Store proof under a dated reports or docs evidence folder when captured. Until then: **not verified for beta**.

## Status snapshot (2026-08-01)

| Item | Status |
| --- | --- |
| Invalid None → schemaVersion error | Fixed (prior polish P0) |
| Interface transparency setting 0–60% | Implemented |
| Nested section/bubble/secondary button overlays | Migrated |
| Remaining `--surface` nested risk | Open — inventory |
| Visual proof pack | **Not done** |
| Public beta claim | **Not ready** |

Machine-readable: `reports/wallpaper-compositing.json`.

## Related

- [SURFACE_COMPOSITING_MODEL.md](./SURFACE_COMPOSITING_MODEL.md)
- [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md)
- [WALLPAPERS.md](./WALLPAPERS.md)
- [READABILITY.md](./READABILITY.md)
