# Wallpaper targeted-update architecture (RC3.2 Phase 2 → RC3.3 Phase 7)

Short vertical slice for atomic workspace appearance writes, plus preview-first wallpaper apply.

## Problem

1. Applying a wallpaper used two sequential `set_setting` calls (`wallpaperJson` then `wallpaper`). A failure between them left inconsistent KV state.
2. The interface-transparency slider called `set_setting` on every `onChange`, flooding IPC/SQLite, with optimistic UI and no rollback.
3. Wallpaper apply awaited Rust persistence before Zustand update, so CSS/renderer lagged behind selection.

## Command

`set_workspace_appearance` (`src-tauri/src/commands/settings_cmds.rs`):

- Input: `{ wallpaperJson?: string, interfaceTransparency?: number }` (camelCase). At least one field required.
- `wallpaperJson: Some(...)` applies the pair with the same clear / legacy / schema rules as the former app-store path; `None` leaves wallpaper keys alone.
- Empty / `{kind:"none"}` clears. Unparseable or unrecognized JSON returns `invalid` (does not silently clear).
- All proposed values are validated with `normalize_setting_kv` **before** any write.
- Writes use `db.with_transaction` + `set_setting_on_conn` so both wallpaper keys (and optional transparency) commit together.

## Frontend

- `api.setWorkspaceAppearance` → single invoke.
- Wallpaper: `applyWorkspaceWallpaper` paints optimistic Zustand (preview) **before** await, tracks last committed pair for rollback, marks committed from settings on success, restores previousCommitted on failure.
- Transparency: `previewInterfaceTransparency` (local) + `commitInterfaceTransparency` (atomic persist + rollback to last committed on failure; same-target in-flight promise reuse for pointerup+blur).
- Slider: preview on change; commit on pointer/key up or blur; presets/Reset commit immediately.
- Settings copy: preview applies immediately; durable save follows.

## Status

Landed vertical slice — not a full wallpaper redesign. Unit-verified preview→commit→rollback; desktop pixel sampling **not_run**. Project wallpapers still use `set_project_wallpaper_cmd`. Agent `settings_change` wallpaper writes now clear `wallpaperJson` in the same SQLite transaction as the legacy `wallpaper` key.
