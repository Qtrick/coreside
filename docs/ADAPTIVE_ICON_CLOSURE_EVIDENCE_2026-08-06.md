# Adaptive Icon Closure Evidence — 2026-08-06/07

## Verdict

P0 adaptive packaging and Split optical parity are implemented and freshly packaged.
Live Dock display capture was denied in the agent session; packaged `Assets.car` contains
Aqua / DarkAqua / Tintable `IconImageStack` entries and `ictool` rendered all six appearances.

## Root causes

1. **No adaptive package** — only static PNG/ICNS; no `Assets.car` / `CFBundleIconName`.
2. **Split geometry** — padded artwork inside the squircle left a transparent ring, so the tile and flower read smaller than Classic.
3. **Stale tooling docs** — claimed `actool` unavailable; live `actool` is 26.6.

## Fixes

| Item | Change |
| --- | --- |
| Split | Unified with Classic outer/squircle/polish fill-then-mask pipeline |
| Adaptive source | `src-tauri/icons/Coreside.icon` |
| Precompile | `scripts/compile_macos_adaptive_icon.sh` → `icons/Assets.car` |
| Bundle | `tauri.conf.json` includes `icons/Assets.car`; `Info.plist` `CFBundleIconName=Icon` |
| Security hardening (reviewer) | Release builds no longer fall back to source-tree dock PNGs |
| Mock contract | Invalid manual combos rejected like Rust |

## Package inspection (fresh build)

- App: `src-tauri/target/release/bundle/macos/Coreside.app`
- Running binary confirmed under that `.app`
- `Contents/Resources/Assets.car` present (~1.77 MB)
- `CFBundleIconName` = `Icon`
- `CFBundleIconFile` = `icon.icns` (static fallback)
- `IconImageStack` appearances: `NSAppearanceNameAqua`, `NSAppearanceNameDarkAqua`, `ISAppearanceTintable`
- `npm run package:scan` → **passed** (0 findings)

## Automated verification

- `python3 scripts/test_branding_icon_geometry.py` — PASS
- `cargo test branding` — pass
- Vitest dock-icon + settings — pass
- `ictool` renditions Default/Dark/ClearLight/ClearDark/TintedLight/TintedDark — OK

## Human visual acceptance

- Evidence strip: `tmp/icon-evidence/manual-tiles-parity.png`
- Renditions: `tmp/icon-evidence/rendition-*.png`
- Live Dock/`screencapture` denied in agent environment — user should cycle System Settings → Appearance → Icon & Widget Style while Follow macOS is selected.

## Remaining risks

- Older macOS (<26) uses static `icon.icns` only.
- Tauri in-bundle `.icon`→`actool` path remains flaky (tauri#15315); we ship precompiled `Assets.car`.
- Live Finder/Launchpad/closed-app visual confirmation still needs a human glance on this machine.
