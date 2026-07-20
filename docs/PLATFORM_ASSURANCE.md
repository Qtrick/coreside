# Platform Assurance

**Product:** Coreside v0.1.0  
**Last updated:** 2026-07-19

## Primary platform

| Platform | Role | Evidence |
| --- | --- | --- |
| **macOS** | Primary development and dogfood target | Maintainer environment; `Info.plist` in bundle config |

Tauri dev: `npm run dev`  
Packaged: `npm run build` (targets `all` in `tauri.conf.json`)

## Secondary platforms (bundle config only)

`tauri.conf.json` includes:

- `icons/icon.ico` (Windows)
- `icons/icon.icns` (macOS)
- `bundle.targets: "all"`

| Platform | Build claimed? | Release evidence in repo? |
| --- | --- | --- |
| Windows | **Not verified** | Icons present; no CI build logs or release artifacts |
| Linux | **Not verified** | No distro-specific docs or test records |

**Do not claim** Windows or Linux release readiness without a recorded packaged build and smoke test on that OS.

## Platform-specific features

| Feature | macOS | Windows | Linux |
| --- | --- | --- | --- |
| OS keychain credential storage | Expected (keyring) | Expected | Expected |
| Secondary native windows | Dev primary | Unknown | Unknown |
| Crawl4AI sidecar | Setup scripts exist | Python dep | Python dep |
| Asset protocol media paths | macOS path in CSP scope | APPDATA path in scope | Not explicitly listed |

## Prerequisites

https://v2.tauri.app/start/prerequisites/

- Node.js 20+
- Rust toolchain for Tauri 2
- Platform WebView dependencies per OS

## Web-only dev mode

`npm run dev:web` runs Vite on port 1422 **without** Tauri shell. Useful for UI iteration; **not** valid for platform assurance (no keyring, no native windows).

## Packaging verification status

| Check | Status |
| --- | --- |
| `npm run build` on macOS | **Pending** for this assurance pack |
| Code signing / notarization | **Not documented** |
| Auto-update | **Out of scope** v0.1.0 |

## Recommendation

Internal alpha: **macOS only**.  
Expand platform claims only after per-OS packaged smoke (A, B, C minimum).
