# Coreside

Coreside is an AI-native personal software environment that begins as a chatbot and builds interactive tools directly inside itself.

You ask for a capability — a water tracker, a checklist, a quiz, an expense splitter — and the Coreside agent proposes a declarative tool that renders in your workspace. Tools persist locally, can be edited through conversation, and can open in a separate native window.

This repository is the **consumer MVP foundation**: a real desktop application with a trusted Rust core, a React interface, local SQLite persistence, and a provider-neutral agent runtime. Gemini is the first implemented AI provider adapter.

## Current consumer MVP scope

- Desktop app via Tauri 2
- BYOK provider setup (Gemini, OpenAI, Anthropic, OpenRouter) with OS credential storage
- **Projects** with instructions, chat membership, context menus, and local FTS project context
- Chat navigation with current-chat no-op and per-chat draft/scroll preservation
- Chat with structured tool proposals (preview → apply / discard) and bounded `tool_use` loop
- Trusted **Web Research** — Exa (optional) for indexed discovery + local Crawl4AI for page inspection, with citations and Safe Search
- **Media Library** with validated imports and source attribution
- Wallpapers (canvas presets + local image/video/animated assets) under Added Settings → Templates
- `@` tool mentions, Action Log Base Setting, local automations, tool export
- Trusted `clock` component with offline HTML export
- Declarative trusted component renderer
- Local SQLite persistence for conversations, tools, versions, settings, automations, projects, media
- System / Light / Dark themes and user-editable workspace appearance
- Secondary native tool windows

Out of scope for this phase: teams, billing, cloud sync, accounts, arbitrary code execution, marketplaces, and enterprise administration.

## Core concepts

| Concept | Meaning |
| --- | --- |
| Agent | The Coreside AI that answers and proposes tools |
| Tool | A personal interactive surface defined as validated JSON |
| Component registry | Trusted UI building blocks the agent may configure |
| Version | Snapshot of a tool definition after each applied change |
| Workspace | Personal container for tools (one default workspace today) |

## Technology stack

- **Desktop:** Tauri 2
- **Trusted core:** Rust
- **Interface:** React + TypeScript + Vite
- **Persistence:** SQLite (bundled via `rusqlite`)
- **Validation:** Serde (Rust) + Zod (TypeScript)
- **State:** Zustand

## Rust and TypeScript responsibilities

### Rust / Tauri core

```text
Coreside protected core
├── Environment and secret loading
├── AI provider adapters
├── Database
├── Native windows
├── Tool-definition validation
├── Logging
└── Application recovery
```

### React / TypeScript interface

```text
Coreside interface
├── Chat
├── Sidebar
├── Settings
├── Tool canvas
├── Tool component registry
├── Preview and undo UI
├── Native-window routes
└── Interactive personal tools
```

The AI generates structured JSON and declarative component definitions — not unrestricted JavaScript, remote script tags, or native Rust.

## Requirements

- Node.js 20+ and npm
- Rust toolchain (rustc / cargo) compatible with Tauri 2
- Platform dependencies for Tauri on your OS ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))
- A Gemini API key for live chat (optional for browsing the shell)

## Installation

```bash
cp .env.example .env
# Paste your key into .env (AI_API_KEY or GEMINI_API_KEY)
npm install
npm run crawl4ai:setup   # local Crawl4AI engine (page inspection)
# Optional: set EXA_API_KEY= in .env for indexed open-web discovery
```

## `.env` configuration

```env
AI_PROVIDER=gemini
AI_API_KEY=
AI_MODEL=
AI_BASE_URL=
CORESIDE_LOG_LEVEL=info

# Provider aliases (used when the matching AI_* value is empty)
GEMINI_API_KEY=
GEMINI_MODEL=
OPENAI_API_KEY=
ANTHROPIC_API_KEY=
CLAUDE_API_KEY=
```

**Precedence:** provider-neutral `AI_*` → aliases for the selected `AI_PROVIDER` → safe non-secret defaults.

You can keep multiple provider keys in `.env` and switch later with `AI_PROVIDER`. Only **Gemini** is implemented today; OpenAI and Anthropic/Claude keys are accepted so future adapters can use them without reshaping `.env`.

Default Gemini model when unset: `gemini-3.8-flash`.

Never commit a populated `.env`. The file is gitignored (`.env` and `.env.*`, with `.env.example` kept).

API keys are loaded only in Rust. They are never sent to the frontend, SQLite, logs, or tool definitions.

## Branding

Coreside uses a protected brand mark:

- **In-app Light:** black transparent mark
- **In-app Dark:** white transparent mark  
- **Dock (OS Light):** dark-background icon with white mark
- **Dock (OS Dark):** light-background icon with black mark

In-app logos follow the Coreside Appearance setting. Dock icons follow the **operating system** appearance even if the in-app theme is overridden. Details: [docs/BRANDING.md](docs/BRANDING.md).

## Running development mode

```bash
npm run dev
```

This launches the Vite frontend and the Coreside desktop window. No Cloudflare account, separate backend process, or manual database migration is required.

Web-only preview (mocks, no desktop shell):

```bash
npm run dev:web
```

## Building the desktop application

```bash
npm run build
```

Frontend-only production bundle:

```bash
npm run build:web
```

## Running tests

```bash
npm run typecheck
npm run lint
npm run test
npm run check:rust
npm run test:rust
npm run verify
```

Automated tests use a mock AI provider / fixtures and do **not** require a paid API key.

## Architecture overview

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Cloud feasibility for research and related systems: [docs/CLOUD_HOSTING.md](docs/CLOUD_HOSTING.md).

**Application Kernel** — trusted gateway for generated apps (manifests, data models, risk proposals, Recovery Mode, `.coreside-app` packages): start at [docs/APPLICATION_KERNEL.md](docs/APPLICATION_KERNEL.md). Related: [APPLICATION_MANIFEST.md](docs/APPLICATION_MANIFEST.md), [CHANGE_COMPILER.md](docs/CHANGE_COMPILER.md), [RECOVERY_MODE.md](docs/RECOVERY_MODE.md), [IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).

Request flow:

1. User message → conversation + optional active tool context
2. System prompts assembled from `src-tauri/prompts/`
3. Provider call (Gemini or mock)
4. Structured response parse + schema validation
5. Chat reply + optional tool-change preview
6. User applies → versioned tool persistence → renderer

## Supported generated components

Layout: `container`, `row`, `column`, `card`, `tabs`, `divider`, `spacer`  
Content: `heading`, `text`, `badge`, `image`, `emptyState`  
Inputs: `textInput`, `textArea`, `numberInput`, `select`, `checkbox`, `dateInput`  
Data: `list`, `checklist`, `table`, `counter`, `progress`, `stat`  
Actions: `button`, `buttonGroup`  
Games: `quiz`

## Current limitations

- Only the Gemini provider adapter is implemented (architecture is provider-neutral)
- Tool edits replace a full declarative snapshot (not fine-grained patches)
- One default personal workspace
- No file/image uploads in chat
- Streaming is secondary to reliable structured tool output

## Security approach

See [docs/SECURITY.md](docs/SECURITY.md).

Summary: secrets stay in Rust; model output is validated; only registry components run; secondary windows share the same restricted capabilities and never receive API keys.

## Partial Update inspiration and attribution

Coreside studied **Partial Update** (MIT, Copyright © 2026 Phil Holden) for generative UI and structured interaction ideas. Unrestricted HTML/JS/CDN injection and Cloudflare-required runtime pieces were intentionally **not** adopted. Generative Interface Runtime V2 adapts the strongest ideas (multi-ops, patches, inline surfaces, packs, branching, queue) through trusted components — see [docs/GENERATIVE_INTERFACE_RUNTIME_V2.md](docs/GENERATIVE_INTERFACE_RUNTIME_V2.md). Final gap work (preservation, patch scheduler, navigation continuity, Customize mode, context ledgers, provider conformance): [docs/PARTIAL_UPDATE_FINAL_GAP_AUDIT.md](docs/PARTIAL_UPDATE_FINAL_GAP_AUDIT.md), [docs/PRESERVATION_ENGINE.md](docs/PRESERVATION_ENGINE.md), [docs/PATCH_SCHEDULER.md](docs/PATCH_SCHEDULER.md). Attribution: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Future direction

See [docs/ROADMAP.md](docs/ROADMAP.md). Next focus: stronger agent harness (intent, context, patch reliability), more components, and safer personal games — not enterprise SaaS features.

## Troubleshooting

| Symptom | What to try |
| --- | --- |
| Send disabled / “not configured” | Put a key in `.env`, save, restart `npm run dev` |
| Desktop window does not open | Confirm Rust + Tauri OS prerequisites; check terminal for cargo errors |
| Theme wrong after restart | Check Settings → Appearance; setting is stored in local SQLite |
| Tool did not appear | Look for a tool-change preview and click Apply |
| Connection test fails | Verify key, model id, and network access to the Gemini API |

## Documentation index

- [Architecture](docs/ARCHITECTURE.md)
- [Application Kernel](docs/APPLICATION_KERNEL.md)
- [Cloud hosting feasibility](docs/CLOUD_HOSTING.md)
- [Agent protocol](docs/AGENT_PROTOCOL.md)
- [Web Research](docs/WEB_RESEARCH.md)
- [Branding](docs/BRANDING.md)
- [Security](docs/SECURITY.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [Partial Update review](docs/PARTIAL_UPDATE_REVIEW.md)
- [Implementation plan](docs/IMPLEMENTATION_PLAN.md)
- [Roadmap](docs/ROADMAP.md)
