# P0.2 Closure Evidence — 2026-08-07

## Human adaptive confirmation (preserved)

The user previously launched:

`/Users/qunyingfan/Coreside/src-tauri/target/release/bundle/macos/Coreside.app`

and confirmed macOS adaptive icon behavior (Default / Dark / Clear / Tinted path) works for the packaged `.app`.

Final P0.2 changes preserved the same architecture (`Coreside.icon` → precompiled `Assets.car` → `CFBundleIconName=Icon`). UI dormancy and Split regeneration do not change adaptive packaging semantics. A fresh `.app` was rebuilt into the project target with source/package `Assets.car` SHA equality. Another human Icon & Widget Style glance is recommended only if visual confirmation of the new binary is desired; structural package verification passed.

## Product decision

- User-facing manual Dock selection removed (Classic Dark / Light / Split / Choose manually / helper copy).
- Implementation retained behind `MANUAL_DOCK_ICON_SELECTION_ENABLED = false` (TS + Rust).
- Migration `024_reset_dock_icon_follow_macos` resets stale manual prefs.
- Runtime/IPC coerce manual → Follow macOS while dormant.

## Split optical closure

- Split rebuilt from canonical Classic Dark + Classic Light marks + shared diagonal mask.
- Outer alpha bbox: `(112,112)-(912,912)` for Dark/Light/Split.
- Shared flower placement: `(198,176)-(825,848)` size `627×672`.
- Contact sheet: `reports/evidence/branding/dock-icon-contact-sheet.png`.

## Adaptive freshness

- Fingerprint file: `src-tauri/icons/Assets.car.fingerprint`
- Command: `npm run brand:verify-adaptive-icon`
- `killall ibtoold` only on known actool failure symptoms (not unconditional).

## Developer workflow

```bash
npm run macos:build-and-run-packaged
npm run macos:run-packaged   # skip build
```

Prints: Adaptive macOS appearance must be tested using this packaged application, not tauri dev.

Canonical package-test path:

`/Users/qunyingfan/Coreside/src-tauri/target/release/bundle/macos/Coreside.app`

Older evidence artifact (do not auto-delete): `tmp/Coreside-adaptive-2026-08-06.app`

## Partial Update

- Rebuilt current-source audit (`scripts/audit-partial-update.mjs`).
- Highest-value gap: progressive generated-surface preview — Unit Verified (+ SQLite cancel/incomplete rollback), still lacking desktop/packaged progressive-paint journey.

## Security (dormancy)

- Manual IPC/commit/apply gated or coerced while capability off.
- Release builds do not resolve branding from `CARGO_MANIFEST_DIR`.
- AppKit mutations remain main-thread hop.
- Dock commands remain main-window-only; tool windows denied.
