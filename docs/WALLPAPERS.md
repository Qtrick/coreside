# Wallpapers

Workspace and project backgrounds with trusted canvas presets and local media.

## Settings UI

Wallpaper templates live under **Added Settings → Templates → Wallpapers** (not Base Settings Appearance). Dock icon / branding stay in Appearance.

Selection is derived from `wallpaperJson` via `parseWallpaperJson` / `activeCanvasPresetId` (`src/lib/wallpaper.ts`). Applying a canvas preset writes schema JSON and resets legacy `wallpaper` to `none` — the UI must not read `wallpaper.kind` alone.

This pass is **Apply-only**: choosing a preset applies immediately. There is no separate `previewWallpaperId` store yet.

## Built-in presets

| Id | Live badge |
| --- | --- |
| `none` | — |
| `matrix`, `aurora`, `particles`, `rain`, `pulse` | Live |

Presets are defined in `CANVAS_PRESETS` (`src/types/wallpaper.ts`). Do not delete built-ins. Agents must not inject arbitrary CSS.

## Schema

See also [LIVE_WALLPAPERS.md](./LIVE_WALLPAPERS.md). `schemaVersion: "1"` types include `canvas-preset`, `floating-particles`, media covers, gradients, and slideshows. Media sources use local `assetId` only (no remote URLs).

## Resolution order

`resolveActiveWallpaper`: project chat / project page override → global `wallpaperJson` → legacy `wallpaper` → none.
