# P0.3 Closure Evidence — 2026-08-08/09

## Objective

Make `npm run dev` on macOS launch a real adaptive `.app` so Icon & Widget Style controls the Dock tile (same Assets.car architecture as release).

## Root cause (verified)

`npm run dev` was bare `tauri dev` → unpackaged Mach-O → `packaged_macos_icon_available() == false` → Classic Dark AppKit stand-in. Release `.app` already adaptive.

## Architecture chosen

**Option A — `tauri dev --runner` packaged hot-dev**

- `npm run dev` → `node scripts/dev.mjs` (darwin packaged; win/linux raw)
- Runner: `scripts/macos-packaged-dev-runner.sh` (cargo-shaped, ends in `exec`)
- Prepare: `scripts/macos-packaged-dev-prepare.mjs`
- Bundle: `src-tauri/target/debug/bundle/macos/Coreside.app`
- Bundle id: `com.coreside.app` (unchanged; app data not keyed by CFBundleIdentifier)
- Escape: `npm run dev:raw`

## Structural proof (Development Desktop Verified — automated)

Observed from `npm run dev` launch log `/tmp/coreside-dev-p03.log`:

- `Running adaptive development bundle: …/target/debug/bundle/macos/Coreside.app`
- `CFBundleIconName: Icon`
- `Assets.car` SHA matched source catalog `994772f2a0ced097bb87384a6fd9cdae913ea55607f8bfed7a9d281289c7b150`
- Runtime log: `Cleared macOS Dock override (Follow macOS, packaged adaptive Assets.car)`
- Classic Dark stand-in log **absent**

## Six-appearance matrix

| Appearance | Result |
| --- | --- |
| Default / Dark / Clear Light / Clear Dark / Tinted Light / Tinted Dark | **Human Blocked** — requires user Icon & Widget Style cycle on the `npm run dev` instance |

## Partial Update

- Journey 19 + mock paint fixture added (`progressive surface preview`)
- Audit hooks for `reports/progressive-preview-results.json`
- Status remains **Unit Verified** until Journey 19 E2E is executed

## Security

- Main-agent review (subagent usage-limited): `reports/security-findings-p0.3-2026-08-08.json`
- Stale CSP `img-src https:` finding superseded (current CSP has no arbitrary https images)
- Sensitive kernel commands main-allowlisted / tool allowlist-isolated (P2 residual)

## Remaining

1. User: run/confirm `npm run dev` Dock adapts across Icon & Widget Style
2. `npm run release:evidence` to clear doctor `evidence.current_commit`
3. Execute Journey 19 for Desktop Verified progressive preview
4. Fresh release `.app` regression after final source freeze
