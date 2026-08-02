# Packaged Build Assurance

**Product:** Coreside

## Local package

```bash
npm run build
npm run package:scan
```

Artifacts land under `src-tauri/target/release/bundle/` (or `$CARGO_TARGET_DIR/release/bundle` when set).

Expected macOS outputs:

- `macos/Coreside.app`
- `dmg/Coreside_*_*.dmg`

## Production must exclude E2E

- Cargo feature `e2e` is **not** in default features
- Production `tauri.conf.json` capabilities: `default` + `tool-window` only (no `wdio`)
- Doctor checks: `e2e.not_in_default_features`, `e2e.lib_plugins_feature_gated`, `e2e.production_config_no_wdio`
- Packaging CI asserts the same before `npm run build`

## CI

`.github/workflows/packaging.yml` builds on macOS, Ubuntu, and Windows without publishing. Artifacts upload for inspection. No production signing secrets are required for ordinary PR verification.

## Contents scan

`npm run package:scan` writes `reports/packaging-results.json` and fails on P0 findings such as `.env`, fixture DBs, or obvious credential snippets inside text bundle files.

## Smoke

Prefer driving the packaged or release-mode binary with the desktop E2E harness where practical (`npm run e2e:desktop` uses a debug E2E build). A successful web build (`npm run build:web`) is **not** equivalent to a packaged Tauri build.
