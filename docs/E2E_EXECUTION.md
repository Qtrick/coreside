# Desktop E2E Execution

## Prerequisites

- macOS (primary; embedded WebDriver provider)
- Node.js 22+
- Rust stable toolchain
- Xcode CLT (for linking)
- Display session (GUI) — headless agents cannot substitute for a real window

Linux/Windows can use the same stack later; CI currently targets macOS.

## Build the E2E binary

```bash
npm ci
npm run e2e:build
```

This runs:

```text
tauri build --debug --features e2e --config src-tauri/tauri.e2e.conf.json --no-bundle
```

Effects:

- Compiles `tauri-plugin-wdio` + `tauri-plugin-wdio-webdriver`
- Builds the web UI with `VITE_E2E=1` (embeds `@wdio/tauri-plugin`)
- Writes `src-tauri/target/debug/Coreside`

Production builds must **omit** `--features e2e` and must **not** pass
`tauri.e2e.conf.json`.

## Run tests

```bash
# Build + all suites (clean profile, then existing-profile)
npm run e2e:desktop

# Suites only (binary already built)
npm run e2e

# CI alias
npm run e2e:ci
```

`e2e/run.mjs` launches isolated sessions:

1. **main** — empty `CORESIDE_DB_PATH` (Journeys 1, 3, 4)
2. **existing-*** — fresh DB + `CORESIDE_E2E_SEED=existing`, one DB per journey group (2, 5–10)

Each seeded journey gets its own temp database so approval/grant state does not leak between specs.

Override binary path if needed:

```bash
export CORESIDE_E2E_BINARY=/absolute/path/to/Coreside
npm run e2e
```

Single suite (advanced):

```bash
CORESIDE_DB_PATH=/tmp/coreside-e2e-clean.db \
  npx wdio run e2e/wdio.conf.ts --suite main
```

## Security reminders

- Do not enable Cargo feature `e2e` in release packaging.
- Do not merge `wdio` permissions into production capabilities.
- Do not add a public “run arbitrary SQL / seed anything” command.
- Prefer `CORESIDE_DB_PATH` isolation; never point E2E at the user profile DB.

## Artifacts

| Artifact | Location |
| --- | --- |
| E2E binary | `src-tauri/target/debug/Coreside` |
| Isolated DB | temp dir under `$TMPDIR/coreside-e2e-*` (per suite) |
| WDIO logs | terminal / CI job log |
| Frontend guest plugin | only when `VITE_E2E=1` |
| Orphan cleanup | `onPrepare` / `onComplete` terminate only processes **listening on WebDriver port 4445** whose executable matches the E2E binary — never broad `pkill` of Coreside |

On suite success, `e2e/run.mjs` removes the isolated temp DB directory. Failed suites leave the DB under `$TMPDIR/coreside-e2e-*` for inspection.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| Binary not found | `npm run e2e:build` or set `CORESIDE_E2E_BINARY` |
| Session timeout | Ensure feature `e2e` was used; embedded server needs `tauri-plugin-wdio-webdriver` |
| `wdioTauri` missing | Rebuild with `VITE_E2E=1` / `tauri.e2e.conf.json` |
| Doctor fails on e2e gates | Production `Cargo.toml` / `tauri.conf.json` must stay free of default `e2e` / `wdio` |
| False “Connected” | Run without hosted session / API keys; suite asserts against this |
| `installMockSyncOverride` import error | `package.json` overrides `@wdio/native-utils` to `2.5.0` (1.2.0 service pins 2.4.0 incorrectly) |
| Sandbox `CARGO_TARGET_DIR` | `e2e:build` runs `env -u CARGO_TARGET_DIR` so the binary lands in `src-tauri/target/` |
| Approval modal blocks tool/settings | Journeys 5, 8–10 call `denyPendingApprovalIfPresent()`; approval journeys use a fresh DB |

## Journey status

| # | Spec | Suite | Coverage | Notes |
| --- | --- | --- | --- | --- |
| 1 | `01-clean-startup.spec.ts` | `main` | **Automated** | Empty isolated DB |
| 2 | `02-existing-profile.spec.ts` | `existing-chat` | **Automated** | Seeded “Biology notes” chat |
| 3 | `03-settings.spec.ts` | `main` | **Automated** | Settings open/close; no false “connected” |
| 4 | `04-new-conversation-draft.spec.ts` | `main` | **Automated** | New chat + composer draft |
| 5 | `05-generated-tool-state.spec.ts` | `existing-tool` | **Automated** | Seeded `tool-e2e-notes` text input persists |
| 6 | `06-approval-approve-once.spec.ts` | `existing-approval` | **Automated** | `approval-e2e-1` → Approve once |
| 7 | `07-multi-window-approval-race.spec.ts` | `existing-approval-race` | **Partial** | Opens secondary window; asserts approval stays on `main` only |
| 8 | `08-grant-revoke.spec.ts` | `existing-grant` | **Automated** | Revokes seeded `grant-e2e-1` in App permissions |
| 9 | `09-recovery-mode.spec.ts` | `existing-recovery` | **Automated** | Enter/exit Recovery Mode |
| 10 | `10-secondary-window.spec.ts` | `existing-window` | **Partial** | Opens `tool-tool-e2e-notes`, switches WebDriver context; close not asserted |

### Seed fixture (`CORESIDE_E2E_SEED=existing`)

When the workspace DB is empty, `src-tauri/src/e2e_support.rs` inserts:

| Resource | Stable ID |
| --- | --- |
| Conversation | `conv-e2e-existing` (“Biology notes”) |
| Personal tool | `tool-e2e-notes` (“E2E Notes”, `textInput` + `note` state) |
| Surface | `surf-tool-e2e-notes` |
| Application manifest | `tool-e2e-notes` |
| Pending approval | `approval-e2e-1` (`local_data.write`, non-critical) |
| Remembered grant | `grant-e2e-1` (`local_data.query`) |

Seed is compile-gated (`e2e` feature) and env-gated — not a public Tauri command.

### Multi-window limitations

- `browser.tauri.listWindows()` / `switchWindow()` work with the embedded provider.
- Journey 7 does **not** assert duplicate approval UI in the secondary window (WebDriver session targets one window at a time).
- Journey 10 verifies open + `browser.tauri.switchWindow()`; **close is not asserted** (bundled frontend cannot reliably call `getCurrentWebviewWindow().close()` from WebDriver execute).
