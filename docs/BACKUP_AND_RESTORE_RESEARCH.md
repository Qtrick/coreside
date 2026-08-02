# Backup and Restore Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Engine:** SQLite via `rusqlite` (`features = ["bundled"]` today — **no `backup` feature yet**)

## Problem

Consumer beta needs a supported way to back up and restore local Coreside data (DB + related app-data). Today:

- Tool/project **export** packages exist (structured JSON / app packages).
- There is **no** first-class full-database backup/restore command for `coreside.db` under the product data dir.
- Raw file copy of a live WAL database is unsafe while the app holds the connection.

Related gap already noted in [DATABASE_READINESS_RESEARCH.md](./DATABASE_READINESS_RESEARCH.md).

## Preferred approaches (SQLite)

| Method | Pros | Cons | Recommendation |
| --- | --- | --- | --- |
| **Online Backup API** (`sqlite3_backup_*`) | Consistent snapshot while DB is open; handles pages safely | Needs rusqlite `backup` feature | **Preferred** for in-app Backup |
| **`VACUUM INTO 'path'`** | Single SQL statement; produces compacted copy | Must run on a connection; destination must be writable | **Valid** alternative / CLI-friendly |
| Raw copy of `coreside.db` (+ WAL/SHM) | Simple | Race with writers; easy to get torn copies | **Reject** as primary product path |

### rusqlite requirement

```toml
rusqlite = { version = "0.32", features = ["bundled", "backup"] }
```

Current `src-tauri/Cargo.toml` has `["bundled"]` only. Enable `backup` before implementing Online Backup API wrappers.

## What to include in a product backup

Minimum consumer bundle (research target — not implemented):

1. SQLite snapshot via Backup API or `VACUUM INTO`
2. Co-located product data under `product_data_dir`: media, chat-attachments, crawler cache policy decision (include vs exclude)
3. Manifest: app version, schema/migration id, created-at, checksums
4. Explicit **exclude** secrets (keyring stays OS-side; never bake keys into backup JSON)

Restore must:

- Refuse to overwrite a live DB without explicit user confirmation
- Validate migration compatibility (or require same/newer app version)
- Re-run migrations only via existing forward-only migrator after replace

## Paths

Authoritative resolution: `db::default_db_path` / `product_data_dir` (`coreside`, legacy `Coreside` reuse). Backup UX should show the resolved folder in plain language under Data settings — not SQL table names.

## Non-claims

- No shipped Backup/Restore UI as of this research date.
- Exporting a single tool/project is **not** a full-profile backup.
- Public beta backup gate remains **open** until a trusted Rust path + manual/E2E evidence exists.

## Next implementation steps

1. Add rusqlite `backup` feature.
2. Implement Rust command: snapshot DB to user-chosen path (Backup API preferred).
3. Optional: `VACUUM INTO` for offline/doctor repair path.
4. Wire Settings → Data → Backup / Restore with confirmations.
5. Record evidence in release gates; never claim pass from unit tests alone.
