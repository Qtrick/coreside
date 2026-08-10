# Coreside P0.4 Implementation Plan — 2026-08-09

## Source identity (pre-work)

| Item | Value |
| --- | --- |
| Branch | `main` |
| Commit | `9ed96b923f158a25f92104b3c600ca551f8a141a` |
| Dirty | Audit-script report rewrites only (baseline) |
| Coreside archive (P0.4) | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` SHA `a9327c01cba67655b5af04ecd0d837b980a5af65348580167f613e6099b8194c` |
| Historical P0.3 archive | SHA `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` (preserve; file may be unavailable) |
| Partial Update archive | `/Users/qunyingfan/Downloads/Partial Update Main.zip` SHA `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |
| Source fingerprint | `c043e4285f505e440fc39cea7f73db389ab0f8d046f3d628f41bcd59a816ae26` |

## Baseline (Auto test-runner)

- Pass: typecheck, lint, vitest 272, dispatcher 22, rust check, rust lib 651, migrations 14, adaptive icon, branding icons, npm audit 0, tauri capabilities, partial-update audit
- Fail: `doctor` (stale release evidence commit), `audit:current-source` (expected archive still P0.3 `7e335f…`)

## P0.3 adaptive status

User confirmed `npm run dev` launches adaptive Dock icon. Record Human Accepted for primary adaptive behavior. Do not redesign Assets.car / Icon Composer / CFBundleIconName unless regression.

## Primary targets

1. Versioned source-intake provenance (`a9327c…` current; preserve `7e335f…`)
2. Chrome/WICG → Partial Update → Coreside crosswalk (inspiration only; no experimental HTML APIs)
3. Run Journey 19 → Desktop Verified progressive preview (fix evidence fingerprint)
4. Fail-closed concurrent Coreside by default; branding exact-mirror; process-path robustness
5. Dev/release isolation decision doc (implement only if low-risk)
6. Kernel sensitive-command defense-in-depth
7. Capability-pack Desktop evidence if Journey 19 stable
8. Fresh release `.app` + package scan
9. Security review (Auto) + code-reviewer-editor (Auto)
10. Evidence freeze / doctor green

## Non-goals

- Chrome `<template for>` as runtime
- Adaptive icon redesign
- Unsafe HTML/JS/CDN ports
- Overwriting `/Applications/Coreside.app`
- Blind dependency upgrades
