# Database Readiness Research

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Status:** Research stub — SQLite local-first readiness  
**Engine:** SQLite via `rusqlite` · migrations `src-tauri/migrations/` (001–015)

## What exists

| Item | Evidence |
| --- | --- |
| Forward-only migrations 001–015 | Contiguous SQL under `src-tauri/migrations/` |
| Fresh install → latest | `db::migration_fixtures` + `npm run test:migrations` |
| Upgrade fixtures (006, 011–014+) | Documented in `docs/MIGRATION_ASSURANCE.md` |
| Project isolation at query layer | Projects + FTS migration 006 |
| Secrets out of SQLite | Provider keys in OS keychain; connection metadata only in DB |
| Registered-action tables | Migration 015 (approvals, grants, audit) |

## Gaps for public beta

| Gap | Risk | Next step |
| --- | --- | --- |
| Large-volume soak (10k+ messages / many tools) | Slow open, FTS bloat | Add fixture + timed open/query benchmark |
| Mid-migration crash UX | User sees opaque failure | Document recover path; test interrupted apply |
| Cross-version package after upgrade | Import/export skew | Manual L/M after upgrade fixture |
| App-data dir path clarity in Settings | Support burden | Plain-language Data category (Settings IA) |
| Backup / export of full DB | Recovery story incomplete | Define supported backup (export tools vs raw DB copy) |
| Concurrent multiwindow writers | Rare corruption / lock contention | Exercise journey K under load |

## Principles (do not regress)

1. Migrations remain **forward-only** and transactional where practical.
2. Agent/runtime must not write secrets into SQLite.
3. Project boundaries enforced in Rust queries, not UI alone.
4. Protected core tables/settings remain non-deletable by agent ops.

## Next steps (ordered)

1. Re-run `npm run test:migrations` at beta gate; attach result to `reports/release-evidence.json`.
2. Add one large-fixture open-time sample (even if informal) before public beta claim.
3. Document user-facing recovery when DB open fails (tie to Recovery Mode / safe startup).
4. After Settings IA: Data section explains local storage in plain words (no SQL table names in primary UI).
5. Defer cloud DB / multi-user sync — out of consumer beta scope.

## Non-claims

- Not a formal SQLite ops audit.
- Not proof that every upgrade path from arbitrary historical installs succeeds.
- Public beta remains **blocked** until migration suite + at least one packaged persistence journey (Z) are recorded.
