# Theme Presets — Architecture (Phase Implementation)

**Status:** Implementation plan for migration `039_theme_presets`  
**Authority:** Rust + SQLite (profile DB). Frontend is not the source of truth.

## Goals

- Users save the **last committed** workspace appearance as named presets.
- Free: 10 presets; Pro: 50 (central policy; only when server entitlement source is trusted).
- Apply is **one SQLite transaction** covering theme preference, all 16 palette keys, wallpaper pair, and interface transparency.
- Apply confirmation is read-only until the user confirms; Cancel/Escape mutate nothing.
- Presets are global workspace visuals; project wallpaper overrides are never silently overwritten.

## Snapshot (schemaVersion 1)

Committed settings only:

- `themePreference`: `system` | `light` | `dark`
- 16 appearance hex fields (light + dark for each semantic token)
- `wallpaper` + `wallpaperJson` pair (same storage semantics as settings)
- `interfaceTransparency` 0–100

No secrets, projects, tools, or binary media. Media wallpapers store validated asset IDs only.

## Storage

Table `theme_presets` in the profile SQLite DB (backup/restore includes it automatically).

Quota enforced inside the insert transaction via `COUNT(*)`.

## Commands (main window only)

`list_theme_presets`, `save_current_theme_preset`, `rename_theme_preset`,
`update_theme_preset_from_current`, `delete_theme_preset`, `apply_theme_preset`,
`get_theme_preset_capabilities`, plus shared `apply_appearance_bundle` for
user-approved appearance proposals (not tool-window callable).

## Entitlements

`theme_presets::policy::max_presets_for_plan(plan_id, source)`:

- `source != "server"` → Free limit (10)
- `pro` → 50 (configurable constant)
- `personal` → 25 (configurable; not marketed as product truth)
- else → 10

Downgrade never deletes presets; only blocks new inserts while over limit.
