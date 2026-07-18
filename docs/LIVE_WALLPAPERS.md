# Live Wallpapers

Schema-driven project and workspace backgrounds in `src-tauri/src/wallpapers/`.

For Templates UI, selection fix, Live badges, and Apply-only behavior, see [WALLPAPERS.md](./WALLPAPERS.md). For surface/contrast tokens, see [READABILITY.md](./READABILITY.md).

## Schema

`schemaVersion: "1"` with types:

- `static-color`, `linear-gradient`, `ambient-gradient`
- `image-cover`, `video-loop`, `animated-image` — require local `assetId` (media library)
- `slideshow` — `slideAssetIds[]`
- `floating-particles`, `canvas-preset` — trusted presets (`matrix`, `aurora`, …)

## Validation

- No raw CSS, `@import`, or `javascript:` in string fields
- No remote `http(s)://` URLs as persistent sources
- Media types must use local `assetId` only

## Project wallpaper

`projects.wallpaper_json` is validated on `set_project_wallpaper_cmd`. Export omits wallpaper JSON (paths and asset ids stay local).

## Global settings

App-wide live wallpaper presets (`matrix`, `aurora`, …) remain in the `wallpaper` settings KV via `settings_change` (see `AGENT_PROTOCOL.md`).

## Renderer

Frontend `LiveWallpaper.tsx` renders global KV presets today; project `wallpaper_json` wiring is backend-ready.
