# Feature Inventory

**Product:** Coreside v0.1.0  
**Last updated:** 2026-07-19  
**Legend:** **implemented** = shipped in repo with persistence/UI; **partial** = present but incomplete or dev-only paths.

Status is evidence-based from source layout, migrations, and `README.md` — not a micro-feature census.

| ID | Area | Status | Evidence (representative) |
| --- | --- | --- | --- |
| `chat` | Chat, streaming, cancel, drafts, scroll preservation | **implemented** | `src/components/chat/`, `runtime_v2` queue/streaming, migration 001 |
| `projects` | Projects, membership, FTS context, isolation | **implemented** | `src/components/projects/`, migration 006, `projects/retrieval.rs` |
| `tools` | Personal tools / surfaces, inline + canvas, versions | **implemented** | `runtime_v2/patch.rs`, migration 012, tool renderer |
| `kernel` | Application Kernel (manifest, packages, compiler, policy) | **partial** | migration 013, `application_kernel/`; recovery UI hiding incomplete |
| `search` | Exa discovery, budgets, Crawl4AI crawl, citations | **partial** | `exa/`, `crawler/`, migrations 009–010; requires optional keys + sidecar setup |
| `media` | Media Library, validation, thumbnails, import approval | **implemented** | `src/components/media/`, migrations 007–008 |
| `wallpapers` | Templates, live assets, readability overlays | **partial** | migrations 005/010, `wallpapers/`, `readability/`; not every wallpaper type may be seeded |
| `automations` | Scheduled automations, pause/cancel, run history | **implemented** | migration 005, `automations/`, `docs/AUTOMATIONS.md` |
| `exports` | Tool / package export, redaction | **implemented** | `exports/`, `docs/EXPORTS.md`, Rust export tests |
| `ai-access-modes` | BYOK, local, developer_environment, hosted, unavailable | **partial** | `ai/access_mode.rs`; **hosted adapter not built** (`hosted_connected: false`) |
| `recovery` | Recovery Mode, safe startup, LKG restore | **partial** | `application_kernel/recovery.rs`, migration 013; surface hiding not fully wired |
| `multiwindow` | Secondary native tool windows | **partial** | Tauri window commands, `docs/MULTIWINDOW_CONCURRENCY.md`; manual K journey pending |

## Cross-cutting systems

| ID | Area | Status | Notes |
| --- | --- | --- | --- |
| `providers` | Gemini, OpenAI, Anthropic, OpenRouter adapters | **implemented** | `src-tauri/src/ai/`, migration 003 |
| `action-log` | Base Setting, sanitized events | **implemented** | migrations 004, 011 |
| `settings` | Base + Added Settings, templates | **implemented** | migrations 002, 005 |
| `runtime-v2` | Operations, transactions, events, branching | **implemented** | migration 012, extensive Rust tests |
| `continuity` | Drafts, preservation, patch scheduler, routes | **implemented** | migration 014, targeted `test:*` scripts |
| `mentions` | `@` tool references | **implemented** | `src/lib/mentions/`, vitest |
| `branding` | Protected logos, icons | **implemented** | `branding/`, `security/protected_resources.rs` |

## Explicitly out of scope (v0.1.0)

- Cloud sync / accounts / billing
- Enterprise admin / SSO / SCIM
- Hosted Coreside AI adapter (designed, not shipped)
- Arbitrary code execution in generated surfaces
- Full WebDriver E2E suite

## Machine-readable export

See `reports/feature-inventory.json`.
