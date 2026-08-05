# Migration Assurance

**Product:** Coreside v0.1.0  
**Location:** `src-tauri/migrations/`  
**Count:** 21 forward migrations (001–021; includes `021_atomic_commit_outbox.sql`)

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
| 015 | `015_registered_actions.sql` | Registered-action runtime: approvals, grants, audit events, build failures |
| 016 | `016_chat_attachments.sql` | Chat attachment binding, GC metadata, message attachment links |
| 017 | `017_tutorial_progress.sql` | Onboarding / tutorial progress persistence |
| 018 | `018_turn_timeline_events.sql` | Turn timeline / inspector event storage |
| 019 | `019_provider_platform.sql` | Provider platform metadata columns (non-secret) |
| 020 | `020_context_ledger_isolation.sql` | Context ledger consume/isolation columns and indexes |
| 021 | `021_atomic_commit_outbox.sql` | Commit event outbox, apply idempotency, turn journal, conversation event log |

## What is verified

| Check | Status |
| --- | --- |
| Migrations apply on fresh install | **Verified** — `db::migration_fixtures::fresh_database_reaches_latest_migration` |
| Upgrade from 006 with seeded user data | **Verified** — projects, chats, messages, tools survive |
| Upgrade from 011 with automations | **Verified** |
| Upgrade from 012 / 013 / 014 | **Verified** — manifests, failed apps, approvals tables |
| Idempotent re-open at latest | **Verified** |
| Foreign-key integrity after upgrade | **Verified** — `PRAGMA foreign_key_check` |
| Contiguous migration numbering | **Verified** + doctor `docs.migration_inventory_complete` |
| Forward-only policy | **Yes** — no down migrations |

Run: `npm run test:migrations`

## What is NOT verified

| Gap | Status |
| --- | --- |
| Large production-like data volume (10k+ messages) | **Not tested** |
| Cross-version export/import after upgrade | **Pending** (manual L/M) |
| Migration failure mid-script recovery UX | **Partial** — DDL+bookkeeping is transactional; user-facing recovery messaging not documented |
| Upgrade stops explicitly seeded through 016–020 | **Partial** — fresh path reaches 020; dedicated mid-stop fixtures for 016–019 remain thin |

## Fixture approach

Fixtures are built programmatically (no committed user databases):

1. `Database::open_path_through(path, "NNN_name")` freezes schema at that migration.
2. Anonymized rows are inserted (biology project, study tool, automations, manifests, approvals).
3. `Database::open_path(path)` applies the remaining migrations.
4. Assertions cover migration list, foreign keys, and repository reads.

## Related docs

- `docs/GENERATED_DATA_MIGRATIONS.md` — generated app data migrations (kernel layer)
- `src-tauri/src/db/mod.rs` — migration application code
- `src-tauri/src/db/migration_fixtures.rs` — upgrade fixture tests

## Honest assessment

Forward migrations are fixture-tested for common upgrade stops (006, 011–015) and fresh installs through `021_atomic_commit_outbox`. Large-volume and export-after-upgrade assurance remain open.
