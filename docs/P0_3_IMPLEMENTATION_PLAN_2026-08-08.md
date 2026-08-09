# Coreside P0.3 Implementation Plan — 2026-08-08

## Source identity (pre-work)

| Item | Value |
| --- | --- |
| Branch | `main` |
| Commit | `590705317ad0cf9b827f847ef1e53506ded931eb` |
| Dirty | `package-lock.json` only (nanoid 3.3.16 → 3.3.18 audit fix) |
| Coreside archive | `/Users/qunyingfan/Downloads/Coreside Chat AI (1).zip` SHA `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` |
| Partial Update archive | `/Users/qunyingfan/Downloads/Partial Update Main.zip` SHA `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |
| Tauri | CLI 2.11.4 / api 2.11.1 / crate 2.11.5 |

## Root cause (verified)

`npm run dev` → bare `tauri dev` → unpackaged Mach-O.

`branding::apply_dock_native` Follow macOS path:

- if `!packaged_macos_icon_available()` → Classic Dark AppKit stand-in
- unpackaged `tauri dev` has no `.app` / `Assets.car`

Release `.app` already participates in adaptive Icon & Widget Style. Do **not** redesign Icon Composer / Assets.car.

## Chosen architecture — Option A

Custom cargo-shaped runner for `tauri dev --runner`:

1. Platform dispatcher: `npm run dev` → `node scripts/dev.mjs`
2. macOS: verify adaptive freshness → `tauri dev --runner scripts/macos-packaged-dev-runner.mjs`
3. Runner accepts cargo-compatible `run … --color always -- [appArgs]`
4. Maps to `cargo build …`, installs into `src-tauri/target/debug/bundle/macos/Coreside.app`
5. Syncs Info.plist, Assets.car, icon.icns, branding resources
6. Ad-hoc `codesign --force --deep -s -`
7. `exec Contents/MacOS/Coreside` so Tauri SharedChild = app PID
8. Windows/Linux: plain `tauri dev`
9. Escape hatch: `npm run dev:raw` → plain `tauri dev`

### Research basis (Tauri CLI 2.11.4 `desktop.rs`)

- Runner argv: `<runner> run [args…] --color always -- [appArgs]`
- cwd: `src-tauri`
- SharedChild is the runner; exiting ends the CLI / Vite
- Therefore runner **must exec** the bundled binary, not `open`

### Bundle identity decision

Keep `CFBundleIdentifier = com.coreside.app`.

App data (`~/Library/Application Support/coreside`) and keychain services are **not** keyed by CFBundleIdentifier. Changing the id would split Dock/LS/TCC identity without isolating data. Mitigate concurrent ambiguity with process-path reporting and safe conflict detection (no broad `killall`).

### Non-goals

- No mutation of release `Coreside.app` during `npm run dev`
- No redesign of adaptive packaging
- No restoring manual Dock UI
- No FFATU interaction
- No recursive `beforeDevCommand` → `npm run dev`

## Phases

### Phase 1 — Packaged hot-dev runner

- `scripts/dev.mjs` platform dispatcher
- `scripts/macos-packaged-dev-runner.mjs` cargo shim + install + exec
- Shared helpers for process identity / Assets.car SHA reporting
- `package.json`: `dev`, `dev:raw`, `dev:bundle-verify`
- Keep `dev:web` = Vite only

### Phase 2 — Runtime classification + packaged script harden

- Add `packaged_macos_adaptive_icon_available()` (Assets.car + bundle)
- Keep Classic Dark stand-in only for true unpackaged/`dev:raw`
- Harden `macos-run-packaged.sh`: conflict detection, exact executable verification, no silent wrong-app activation

### Phase 3 — Tests / audits / security / Partial Update

- Dispatcher + command-graph tests (no recursion)
- Bundle prep / freshness / process-identity tests
- Update `audit-current-source.mjs` provenance for archive `7e335f18…`
- Fix doctor migration inventory (`024`)
- Regenerate evidence / security review / Partial Update progressive-preview work
- code-reviewer-editor on final diff

## Acceptance

P0 closed when `npm run dev` on macOS launches the debug `.app` with current Assets.car, correct CFBundleIconName, no Classic Dark stand-in, and the running PID belongs to that bundle. Six-appearance Dock cycling remains Human Accepted until the user confirms on the `npm run dev` instance.
