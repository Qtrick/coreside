# Database bootstrap and recovery

**Product:** Coreside  
**Access date:** 2026-08-02  
**Status:** Initial no-panic bootstrap shipped; full restore/quarantine UX still deepening

## Problem

`Database::open_default()` previously panic’d in `lib.rs` when open or migration failed, which could prevent the protected UI from appearing.

## Current design

1. `db::open_profile_or_shell()` attempts the profile database.
2. On failure, opens a fresh recovery shell database under `<product_data_dir>/recovery/shell.db` (or `:memory:` as last resort).
3. `AppState.bootstrap` records `Ready` or `RecoveryRequired`.
4. Frontend `getBootstrapStatus` / `BootstrapRecoveryScreen` shows Retry without claiming the shell DB is the user profile.
5. Profile-gated commands (`send_message`, `clear_*`, backups, etc.) call `AppState::require_profile()` and return `database_unavailable`.

## Production path policy

Repository-relative `.coreside` fallback is allowed only for `debug_assertions` or `CORESIDE_ALLOW_REPO_DB=1`.

## Remaining work

- Quarantine corrupt profile files with hashes
- Full `.coreside-backup` archive restore with staging/rollback
- Packaged failure smoke evidence on current commit
