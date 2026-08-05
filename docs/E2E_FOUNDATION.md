# Desktop E2E Foundation

Coreside ships a **real** desktop E2E harness on macOS using the official
WebdriverIO + Tauri stack (not IPC-only stubs).

## Stack (pinned)

| Layer | Package | Version |
| --- | --- | --- |
| Rust plugin (execute / mock) | `tauri-plugin-wdio` | `1.2.0` |
| Rust embedded WebDriver | `tauri-plugin-wdio-webdriver` | `1.2.0` |
| npm service | `@wdio/tauri-service` | `1.2.0` |
| npm frontend guest | `@wdio/tauri-plugin` | `1.2.0` |
| WebdriverIO | `webdriverio` / `@wdio/cli` | `9.30.0` |
| native-utils override | `@wdio/native-utils` | `2.5.0` (service 1.2.0 pins 2.4.0 incorrectly) |

Driver provider: **`embedded`** (required for native macOS without CrabNebula).

## Security gates

1. Cargo feature `e2e` is **not** in `default` features.
2. Production `tauri build` / `cargo build` without `--features e2e` does **not**
   compile or register WebDriver plugins.
3. Production `src-tauri/tauri.conf.json` capabilities are only
   `default` + `tool-window` (no `wdio:*`).
4. E2E builds use `src-tauri/tauri.e2e.conf.json`, which adds inline `wdio`
   permissions and `withGlobalTauri`.
5. Frontend loads `@wdio/tauri-plugin` only when `VITE_E2E=1`.
6. Isolated profile via existing `CORESIDE_DB_PATH` (see `db::default_db_path`).
7. Optional seed via `CORESIDE_E2E_SEED=existing` — compile-gated helper in
   `e2e_support.rs`, **not** a public Tauri command.

Doctor enforces the production-side gates (`npm run doctor`).

## Journeys

| # | Spec | Assertion |
| --- | --- | --- |
| 1 | `e2e/specs/01-clean-startup.spec.ts` | App boots on empty DB; no seeded chat |
| 2 | `e2e/specs/02-existing-profile.spec.ts` | Seeded “Biology notes” visible |
| 3 | `e2e/specs/03-settings.spec.ts` | Settings open/close; no false “connected” |
| 4 | `e2e/specs/04-new-conversation-draft.spec.ts` | New chat + draft without live provider |
| 5 | `e2e/specs/05-generated-tool-state.spec.ts` | Seeded tool text input persists |
| 6 | `e2e/specs/06-approval-approve-once.spec.ts` | Approve once dismisses pending approval |
| 7 | `e2e/specs/07-multi-window-approval-race.spec.ts` | **Partial** — secondary absence asserted; concurrent cross-window race not exercised |
| 8 | `e2e/specs/08-grant-revoke.spec.ts` | Revoke seeded remembered grant |
| 9 | `e2e/specs/09-recovery-mode.spec.ts` | Enter/exit Recovery Mode |
| 10 | `e2e/specs/10-secondary-window.spec.ts` | **Partial** — open + switch; close not asserted |
| 11 | `e2e/specs/11-command-authority-denial.spec.ts` | Tool-window command authority denial |
| 12 | `e2e/specs/12-true-streaming.spec.ts` | True provider streaming (mock live probe) + identity evidence |
| 13 | `e2e/specs/13-wallpaper-targeted-update.spec.ts` | Wallpaper apply + transparency + canvas pixel sampling |
| 14 | `e2e/specs/14-stream-eavesdropping-denial.spec.ts` | Tool-window Text eavesdropping denial |

## How to run

See **[E2E_EXECUTION.md](./E2E_EXECUTION.md)**.

```bash
npm run e2e:desktop   # build e2e binary + run all suites
npm run e2e:ci        # same entry used by CI
```

## Status (2026-08-01)

| Item | Status |
| --- | --- |
| Harness code | **Implemented** (`e2e/`, Cargo feature `e2e`) |
| CI E2E job | **Configured** (`.github/workflows/e2e-desktop.yml`, macOS) |
| Migration fixture stand-in | Still covered by `npm run test:migrations` |
| Multi-window journeys (7, 10) | **Partial** — see `docs/E2E_EXECUTION.md` |
