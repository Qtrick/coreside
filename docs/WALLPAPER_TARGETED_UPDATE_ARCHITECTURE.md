# Wallpaper targeted-update architecture (RC3.2 Phase 2)

Short vertical slice for atomic workspace appearance writes.

## Problem

1. Applying a wallpaper used two sequential `set_setting` calls (`wallpaperJson` then `wallpaper`). A failure between them left inconsistent KV state.
2. The interface-transparency slider called `set_setting` on every `onChange`, flooding IPC/SQLite, with optimistic UI and no rollback.

## Command

`set_workspace_appearance` (`src-tauri/src/commands/settings_cmds.rs`):

- Input: `{ wallpaperJson?: string, interfaceTransparency?: number }` (camelCase). At least one field required.
- `wallpaperJson: Some(...)` applies the pair with the same clear / legacy / schema rules as the former app-store path; `None` leaves wallpaper keys alone.
- All proposed values are validated with `normalize_setting_kv` **before** any write.
- Writes use `db.with_transaction` + `set_setting_on_conn` so both wallpaper keys (and optional transparency) commit together.

## Frontend

- `api.setWorkspaceAppearance` → single invoke.
- `applyWorkspaceWallpaper` updates Zustand only after success.
- Transparency: `previewInterfaceTransparency` (local) + `commitInterfaceTransparency` (atomic persist + rollback to last committed on failure).
- Slider: preview on change; commit on pointer/key up or blur; presets/Reset commit immediately.

## Status

Landed vertical slice — not a full wallpaper redesign. Project wallpapers still use `set_project_wallpaper_cmd`. Agent `settings_change` wallpaper writes now clear `wallpaperJson` in the same SQLite transaction as the legacy `wallpaper` key.
