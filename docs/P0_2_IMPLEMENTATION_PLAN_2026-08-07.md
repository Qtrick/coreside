# Coreside P0.2 Implementation Plan — 2026-08-07

## Current source identity (pre-work)

- Branch: `main` @ `57f65ae3475b1fd2a1976d0ef329a532e20078ba`
- Dirty: FFATU deletions (ignored), `src-tauri/icons/Assets.car` modified
- Partial Update archive: `/Users/qunyingfan/Downloads/Partial Update Main.zip` SHA `8666c226…`
- No migration 024; dock persistence ends at `023_dock_icon_preference`
- Manual Dock UI is live and ungated in `SettingsPanel.tsx`
- Adaptive packaging (Coreside.icon → Assets.car → CFBundleIconName=Icon) is known-good when launched as packaged `.app`

## Critical constraints

1. Do **not** replace the adaptive icon architecture.
2. Remove user-facing manual selection; preserve dormant implementation.
3. Stale persisted manual prefs must not block Follow macOS.
4. Polish Split flower geometry from canonical Classic marks, not independent precomposed scale.
5. Ignore FFATU entirely.

## Phases

### Phase 1 — Capability gate + UI dormancy

- Add compile-time/source const `MANUAL_DOCK_ICON_SELECTION_ENABLED = false` (TS + Rust, mirrored).
- Extract manual selector UI into `ManualDockIconSelector.tsx` (retained, not mounted).
- Remove entire user-facing Dock icon section from Appearance while gate is off.
- Update Appearance category copy/search keywords (Theme + wallpapers; no Dock selection UX).
- Clean accessibility tree / pending UI / helper strings from active product.

### Phase 2 — Runtime & persistence authority

- Prefer Option A+C hybrid:
  - Runtime: while gate disabled, effective authority always `follow_macos` (clear AppKit override / packaged adaptive).
  - Migration 024: reset stored `dockIcon` to Follow macOS so reactivation starts clean.
- Gate `commit_dock_icon_preference` / `apply_persisted_dock_icon` so manual mutations cannot activate while disabled.
- Keep startup ownership: Rust owns native mutation; main-window frontend triggers one apply after bootstrap; apply must not reassert stale manual.

### Phase 3 — Split optical closure

- Rebuild Split from canonical Classic Dark + Classic Light + shared diagonal mask.
- Keep outer tile parity tests; add inner flower geometry metrics (bbox, centroid, opening, petal extents).
- Generate multi-scale contact sheet evidence (not Settings cards).

### Phase 4 — Adaptive packaging reliability

- Fingerprint Icon Composer sources + verify Assets.car freshness (`brand:verify-adaptive-icon`).
- Narrow `killall ibtoold` to failure-recovery only.
- Add `macos:build-and-run-packaged` (or equivalent) developer workflow; never overwrite `/Applications`.
- Fresh `tauri build --bundles app` + package scan + plutil/assetutil checks.

### Phase 5 — Tests / docs / audits / security / Partial Update

- Vitest: Settings absence of manual UI; capability default false.
- Rust + Python: dormant gate, stale preference, Split flower parity, adaptive freshness.
- Docs: BRANDING.md + adaptive research notes updated for product decision + dormant reactivation.
- Harden `audit-current-source.mjs` statuses; rebuild Partial Update current-source audit.
- Security review of IPC dormancy, packaged resource resolution, AppKit main-thread.
- Specialist agents: code-reviewer-editor, test-writer, security-review.

## Non-goals

- No custom System Settings polling.
- No six-PNG runtime swap for adaptive appearances.
- No Tauri major upgrade solely for icon features.
- No `/Applications` overwrite without explicit user permission.
- No FFATU interaction.
