# Project Context

Project-scoped memory for chats assigned to a `project_id`.

## Settings

| Key | Default | Description |
| --- | --- | --- |
| `includeProjectContext` | `true` | Inject project instructions + FTS snippets into agent prompt |
| `safeSearch` | `standard` | Also used for web/image/video search |

## Retrieval

`search_project_context` (FTS5 `message_fts`) returns snippets only from conversations in the same project. Stale index rows are filtered with an `EXISTS` membership check.

## Prompt assembly

When `send_message` runs on a conversation with `project_id` and `includeProjectContext` is enabled:

1. Load project instructions and summary
2. FTS-search using the user's message text (top 6 hits)
3. Append a `## Project context` block via `build_agent_prompt_with_references`

Action Log emits `project_context_loaded` when enabled.

## Commands

- `search_project_context_cmd` — explicit search from UI
- `refresh_project_summary` — regenerate summary
- `rebuild_project_index_cmd` — reindex FTS after moves

See `PROJECTS.md` for CRUD and chat assignment.
