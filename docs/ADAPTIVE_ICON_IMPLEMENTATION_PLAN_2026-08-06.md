# Adaptive Icon + Split Parity — Implementation Plan (2026-08-06)

## Goals (P0)

1. Real macOS adaptive packaged icon (Default / Dark / Clear / Tinted).
2. Follow macOS uses packaged icon system-wide (Dock, Finder, Applications, Launchpad, closed).
3. Split manual tile optical parity with Classic Dark / Classic Light.
4. Fix Critical/High security issues in affected paths only.
5. Fresh package + evidence — not PNG/dev-only claims.

## Phases

| # | Work | Status |
| --- | --- | --- |
| 1 | Inspect source, archives, tooling | done |
| 2 | Research Apple/Tauri + actool 26.6 | done |
| 3 | Fix Split geometry in generator | done |
| 4 | Author `Coreside.icon` + precompile `Assets.car` | done (source + precompile) |
| 5 | Wire `tauri.conf.json` / `Info.plist` / docs | done |
| 6 | Tests (generator geometry, packaging metadata) | done |
| 7 | Security review of dock/bundle paths | done |
| 8 | Code-reviewer-editor on full diff | done |
| 9 | Fresh `tauri build --bundles app` + inspect | **pending** |
| 10 | Visual verification + evidence report | **pending** |

## Concrete file changes

- `scripts/generate_branding_assets.py` — unify Split with Classic pipeline
- `scripts/compile_macos_adaptive_icon.sh` — reproducible `actool` → `Assets.car`
- `src-tauri/icons/Coreside.icon/` — Icon Composer package
- `src-tauri/icons/Assets.car` — precompiled adaptive catalog
- `src-tauri/tauri.conf.json` — add `icons/Assets.car`
- `src-tauri/Info.plist` — `CFBundleIconName` = `Icon`
- Docs: `BRANDING.md`, icon-composer README, research doc
- Tests: Python geometry asserts + packaging scan expectations

## Out of scope

- Tauri major upgrade
- Partial Update feature ports
- Runtime bundle mutation

## Honesty gate

Do not mark goal 1/2 complete until phase 9–10 succeed on a fresh `.app` bundle.
Source wiring + precompiled `Assets.car` in the repo is necessary but not packaged verification.
