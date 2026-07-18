# Architecture

## Overview

Coreside is a hybrid desktop application:

- **Rust (Tauri)** owns secrets, networking to AI providers, SQLite, validation, logging, and native windows.
- **TypeScript (React)** owns chat UX, settings, tool rendering, local interactive state, and preview/undo UI.

```text
User
  ↓
React shell (chat / sidebar / tool canvas / settings)
  ↓  Tauri IPC (no API keys)
Rust commands
  ↓
AI provider adapter  ·  SQLite  ·  Web Research sidecar (Crawl4AI stdio)  ·  window manager
```

## Rust / TypeScript boundary

| Concern | Owner |
| --- | --- |
| `.env` / API keys | Rust |
| Gemini HTTP | Rust |
| Agent prompt assembly | Rust (`src-tauri/prompts/`) |
| Response parse + schema validate | Rust (frontend may re-validate for UX) |
| Persistence | Rust / SQLite |
| Component rendering | TypeScript |
| Action engine | TypeScript |
| Theme application | TypeScript (preference persisted via Rust) |

## Frontend structure

```text
src/
├── app/                 App bootstrap + hash routes for tool windows
├── components/
│   ├── chat/
│   ├── context-menu/
│   ├── layout/
│   ├── media/
│   ├── projects/
│   ├── search/
│   ├── settings/
│   ├── sidebar/
│   ├── tool-canvas/
│   ├── wallpaper/
│   └── tool-renderer/   Registry + nodes (including quiz)
├── hooks/
├── lib/                 Tauri API, navigation, media, wallpapers
├── stores/              Zustand app store (AppView navigation)
├── styles/              Design tokens + global CSS
└── types/
```

## Rust module structure

```text
src-tauri/src/
├── ai/           Providers, prompts, parser, schema, tool_loop, capabilities
├── projects/     Projects CRUD, FTS indexing, retrieval, summaries, export
├── search/       Research registry, SSRF-safe fetch, citations (legacy Brave storage readable)
├── research/     Crawl4AI SearchProvider adapter
├── crawler/      Sidecar supervisor, installation probe, resource profiles
├── media/        Import validation, storage, metadata
├── wallpapers/   Trusted wallpaper schemas + validation
├── automations/  Scheduler + executor
├── credentials/  Keyring resolve for AI providers
├── commands/     Tauri IPC surface
├── config/       Env loading
├── db/           SQLite + repositories
├── security/     Redaction + protected resources
├── windows/      Secondary tool windows
├── state.rs
├── lib.rs
└── main.rs
```

## Database architecture

- File: platform app-data directory under `coreside/coreside.db`
- Migrations run automatically on startup (`src-tauri/migrations/` — through `011_action_log_mode`)
- Tables include: `settings`, `workspaces`, `conversations`, `messages`, `tools`, `tool_versions`, `tool_state`, `projects`, `chat_summaries`, `message_fts`, `media_assets`, `search_sessions`, `search_results`, `exa_usage_ledger`, crawler tables, automations, provider_connections
- Tool apply/undo use transactions so malformed mid-writes cannot half-update tools

## Cloud hosting (future)

Desktop Coreside is not lifted wholesale into the cloud. Exa search, research orchestration, and usage/budgets are the strongest cloud candidates; Crawl4AI needs a worker fleet; keyring, Dock, and live wallpapers stay on-device. Full analysis: [CLOUD_HOSTING.md](./CLOUD_HOSTING.md).

## AI request flow

1. Persist user message
2. Select recent conversation messages + optional active tool definition
3. Assemble system prompt (`coreside-prompt-v1`)
4. Call `AiProvider::chat`
5. Parse JSON → validate schema
6. Persist assistant message; store tool change as **pending** metadata
7. Frontend shows preview; Apply creates a new tool version

## Tool renderer

The agent configures trusted components by type + props + actions. The registry maps types to React implementations. Unknown types render a safe fallback. Actions are validated and scoped to the current tool state map.

## Versioning

Each applied tool change writes a new `tool_versions` row and updates `tools.current_version`. Undo restores the previous definition and advances version history without deleting unrelated data.

## Native windows

`open_tool_window` creates or focuses `tool-{id}` loading `/#/tool/{id}`. The secondary window uses the same renderer and persisted state, shares default capabilities, and never receives secrets.

## Future extension points

- Additional `AiProvider` implementations
- Multiple workspaces
- Component-level patches
- Sandboxed custom components (explicitly out of MVP)
