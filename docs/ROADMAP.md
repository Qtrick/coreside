# Roadmap

## Shipped in recent consumer phases

- BYOK AI providers + Action Log + mentions + automations + exports
- Projects, project context (FTS), context menus, chat navigation no-op
- Web Research via local Crawl4AI sidecar (web/image/video discovery), citations, SSRF-safe page fetch
- Media Library + live wallpapers with local assets
- Bounded agent `tool_use` loop with approval for media imports

## Next recommended development

- Full interactive A–AB / AD manual acceptance on a real Tauri build (`docs/MANUAL_ACCEPTANCE_A_AB.md`)
- Deeper caret/selection preservation instrumentation
- Local provider conformance benchmark runs (record real measurements only)
- Project file uploads and multimodal project context
- Better semantic retrieval / optional local embeddings
- Voice / Wasm sandbox / enterprise connectors (only after acceptance passes)

## Later custom tools and games

- Expanded component library
- Controlled animation / audio primitives
- Template starters for common personal tools

## Later cloud synchronization

- **Research-first cloud** (Exa + orchestration + budgets; Crawl4AI as workers) — see [CLOUD_HOSTING.md](./CLOUD_HOSTING.md)
- Optional encrypted sync of conversations and tools
- Device pairing — still personal-first

## Later teams

- Shared workspaces
- Lightweight permissions

## Later enterprise capabilities

- Admin distribution, governance, procurement, audit — only after the personal foundation is solid

Do not treat roadmap items as incomplete MVP requirements.
