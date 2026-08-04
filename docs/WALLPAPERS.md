# Wallpapers

Workspace and project backgrounds with trusted canvas presets and local media.

## Settings UI

Wallpaper templates live under **Added Settings → Templates → Wallpapers** (not Base Settings Appearance). Dock icon / branding stay in Appearance.

Selection is derived from `wallpaperJson` via `parseWallpaperJson` / `activeCanvasPresetId` (`src/lib/wallpaper.ts`). Applying a canvas preset writes schema JSON and resets legacy `wallpaper` to `none` — the UI must not read `wallpaper.kind` alone.

Preset selection is **preview-first**: Zustand (and therefore the live renderer) updates immediately; durable `set_workspace_appearance` follows. Persist failure restores the last committed wallpaper. There is no separate `previewWallpaperId` field — preview reuses the same wallpaper store fields.

## Atomic appearance writes (RC3.2 Phase 2)

Workspace wallpaper + interface transparency use `set_workspace_appearance` — one transactional SQLite write for the wallpaper pair (`wallpaperJson` + `wallpaper`) and/or transparency. Wallpaper apply is preview-first (optimistic Zustand before IPC); failure rolls back to the last persisted pair.

The transparency slider previews locally on `onChange` and commits once on `pointerup` / `keyup` / `blur` (or immediately for presets/Reset). Commit failure rolls the UI back to the last persisted value.

See [WALLPAPER_TARGETED_UPDATE_ARCHITECTURE.md](./WALLPAPER_TARGETED_UPDATE_ARCHITECTURE.md).

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
