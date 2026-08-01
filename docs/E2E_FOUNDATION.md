# Desktop E2E Foundation

Coreside does not yet ship a green desktop E2E suite. This document is the
foundation so the first journeys can be added without inventing harness choices
under release pressure.

## Recommended approach (Tauri 2)

Prefer a **trusted IPC integration harness** over full WebDriver initially:

1. Launch the app with `CORESIDE_DB_PATH` pointed at a temp fixture database.
2. Drive journeys through the same Tauri commands the UI calls (`create_conversation`, `send_message`, `kernel_invoke_registered_action`, …).
3. Assert SQLite state and emitted events.

GUI WebDriver (`tauri-driver`) remains desirable for Journeys that need real focus, multi-window, and keyboard trapping — but it is brittle across platforms and is **not** configured in this repository yet.

## Seeded journeys to automate first

| # | Journey | Minimum assertion |
| --- | --- | --- |
| 1 | App starts against empty DB | migrations = 15, workspace seeded |
| 2 | Create conversation | row exists; sidebar list command returns it |
| 3 | Open settings | `get_settings` succeeds |
| 4 | Tool canvas from seeded tool | `get_tool` returns definition |
| 5 | Seeded pending approval | `kernel_list_pending_approvals` → decide → consumed |
| 6 | Secondary tool window | `open_tool_window` succeeds on platforms that support it |

## Local commands (when harness lands)

```bash
# Placeholder — harness not implemented yet
npm run test:e2e
```

Until then, migration fixtures and unit/integration tests cover the data path:

```bash
npm run test:migrations
npm run test:registered-actions
npm run verify
```

## Manual desktop checklist

See `docs/MANUAL_RELEASE_CHECKLIST.md`. Do **not** mark GUI journeys passed from agent runs without a display.

## Status (2026-08-01)

| Item | Status |
| --- | --- |
| Harness code | **Not implemented** |
| CI E2E job | **Not configured** |
| Migration fixture stand-in | **Implemented** (`npm run test:migrations`) |
| Multi-window approval on desktop | **Manual verification required** |
