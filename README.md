# Coreside

Coreside is an AI-native personal software environment that begins as a chatbot and builds interactive tools directly inside itself.

You ask for a capability — a water tracker, a checklist, a quiz, an expense splitter — and the Coreside agent proposes a declarative tool that renders in your workspace. Tools persist locally, can be edited through conversation, and can open in a separate native window.

This repository is the **consumer MVP foundation**: a real desktop application with a trusted Rust core, a React interface, local SQLite persistence, and a provider-neutral agent runtime. Gemini is the first implemented AI provider adapter.

## Current consumer MVP scope

- Desktop app via Tauri 2
- Chat with a real AI provider (Gemini)
- Structured tool proposals with preview → apply / discard
- Declarative trusted component renderer (utilities + quiz)
- Local persistence for conversations, tools, versions, and tool state
- Undo for the latest tool change
- System / Light / Dark themes
- Open a tool in a secondary native window
- Missing API-key setup guidance without crashing

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

Default Gemini model when unset: `gemini-3.5-flash`.

Never commit a populated `.env`. The file is gitignored (`.env` and `.env.*`, with `.env.example` kept).

API keys are loaded only in Rust. They are never sent to the frontend, SQLite, logs, or tool definitions.

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

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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

Coreside studied **Partial Update** (MIT, Copyright © 2026 Phil Holden) for generative UI and structured interaction ideas. Unrestricted HTML/JS/CDN injection and Cloudflare-required runtime pieces were intentionally **not** adopted. Details: [docs/PARTIAL_UPDATE_REVIEW.md](docs/PARTIAL_UPDATE_REVIEW.md).

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
- [Agent protocol](docs/AGENT_PROTOCOL.md)
- [Security](docs/SECURITY.md)
- [Partial Update review](docs/PARTIAL_UPDATE_REVIEW.md)
- [Implementation plan](docs/IMPLEMENTATION_PLAN.md)
- [Roadmap](docs/ROADMAP.md)
