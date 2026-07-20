# Migration Assurance

**Product:** Coreside v0.1.0  
**Location:** `src-tauri/migrations/`  
**Count:** 14 forward migrations (001–014; no 015+)

## Inventory

| # | File | Summary |
| --- | --- | --- |
| 001 | `001_initial.sql` | Core schema: settings, workspaces, conversations, messages, tools, versions |
| 002 | `002_added_settings.sql` | Added Settings (user/tool-owned, not `core.*`) |
| 003 | `003_provider_connections.sql` | Provider connection metadata (keys in keychain only) |
| 004 | `004_action_log.sql` | Action Log base setting + sanitized events |
| 005 | `005_automations.sql` | Automations, run history, workspace background presets |
| 006 | `006_projects.sql` | Projects, chat membership, summaries, local FTS |
| 007 | `007_media_search.sql` | Media library + search session metadata |
| 008 | `008_media_thumbnails.sql` | Media thumbnail column |
| 009 | `009_crawler.sql` | Crawl4AI engine tables (jobs, sources, cache) |
| 010 | `010_exa_wallpapers.sql` | Exa usage ledger, research budget settings, wallpaper stub |
| 011 | `011_action_log_mode.sql` | `actionLogMode` off \| always \| intelligent |
| 012 | `012_runtime_v2.sql` | Runtime V2: surfaces, transactions, events, branches, queue |
| 013 | `013_application_kernel.sql` | Application Kernel: manifests, packages, permissions, recovery |
| 014 | `014_continuity_scheduler.sql` | Preservation, drafts, patch scheduler, routes, context ledger |

## What is verified

| Check | Status |
| --- | --- |
| Migrations apply on fresh install | **Implicit** — app boots in dev with empty DB |
| Idempotent statements where noted | **Partial** — e.g. 012 lazy tool→surface wrap |
| Rust DB module tests | **Partial** | `src-tauri/src/db/mod.rs` has tests |
| Forward-only policy | **Yes** — no down migrations |

## What is NOT verified

| Gap | Status |
| --- | --- |
| Fixture DB at 001 → upgrade to 014 | **Pending** |
| Large production-like data volume | **Not tested** |
| Cross-version export/import after upgrade | **Pending** (manual L/M) |
| Migration failure recovery | **Not documented** |

## Recommended fixture testing (future)

1. Commit anonymized SQLite snapshots at milestones (006, 012, 013).
2. CI job: copy fixture → run migration runner → assert schema version + row counts.
3. Record results in `reports/release-gates.json` under `migration_fixtures`.

## Related docs

- `docs/GENERATED_DATA_MIGRATIONS.md` — generated app data migrations (kernel layer)
- `src-tauri/src/db/mod.rs` — migration application code

## Honest assessment

Migrations exist and ship with the app. **Upgrade assurance from real user databases is not yet automated.**
