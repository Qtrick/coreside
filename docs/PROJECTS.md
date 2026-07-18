# Projects

Projects group chats, instructions, summaries, and optional wallpapers.

## Data model

Table `projects` (migration `006`): name, description, icon, instructions, summary, `wallpaper_json`, archive/pin timestamps.

`conversations.project_id` nullable FK — `ON DELETE SET NULL` (chats are never silently deleted with the project unless `DeleteChats` mode).

## Commands

CRUD: `list_projects_cmd`, `get_project_cmd`, `create_project_cmd`, `update_project_cmd`, `archive_project_cmd`, `restore_project_cmd`, `delete_project_cmd`.

Chat assignment: `assign_conversation_to_project_cmd`, `assign_chats_to_project`, `remove_chat_from_project`, `list_project_conversations_cmd`, `list_unassigned_conversations_cmd`.

Context: `search_project_context_cmd`, `refresh_project_summary`, `rebuild_project_index_cmd`.

Wallpaper: `set_project_wallpaper_cmd` (validated JSON).

Export: `export_project` — redacts secrets; omits `wallpaper_json`.

## Protected resources

`core.projects`, `core.projects.index`, `core.projects.retrieval`, `core.projects.summaries` — agent cannot mutate via tool changes.

## Related docs

- `PROJECT_CONTEXT.md` — prompt injection and FTS
- `LIVE_WALLPAPERS.md` — `wallpaper_json` schema
