# Branding

## Supplied sources

Inspected from the workspace upload set:

| Supplied file | Role | Findings |
| --- | --- | --- |
| `Coreside Black Logo.png` | Dock source for OS Light (dark bg + white mark) | RGB, solid near-black background, real mark |
| `Coreside White Logo.png` | Dock source for OS Dark (light bg + black mark) | RGB, solid near-white background, real mark |
| `Coreside Black Transparent Logo.png` | Intended black in-app mark | **No alpha channel** — checkerboard / light bg baked into RGB |
| `Coreside White Transparent Logo.png` | Intended white in-app mark | **No alpha channel** — checkerboard / gray bg baked into RGB |

## Transparency correction

Fake “transparent” files were **not** used for production assets.

Genuine RGBA marks were derived from the solid-background sources:

1. **White transparent mark** — soft luminance mask from the black-background white symbol; background crushed to alpha 0; mark forced to pure white with soft anti-aliased edges.
2. **Black transparent mark** — inverse luminance mask from the white-background black symbol; same soft-edge treatment.

Validation (corners alpha 0, RGBA mode, substantial transparent + opaque pixel counts) is performed by `scripts/generate_branding_assets.py`.

## Final asset mapping

### In-app (follows Coreside Appearance)

| Appearance | Asset |
| --- | --- |
| Light | `src/assets/branding/coreside-mark-black-transparent.png` (also `public/branding/`) |
| Dark | `src/assets/branding/coreside-mark-white-transparent.png` |
| System | Follows live OS preference via the resolved light/dark appearance |

Manifest: `src/assets/branding/manifest.ts` (`inAppLogoFor`).

### Dock / application icons

**CURRENT PRODUCT (P0.3):** Coreside automatically follows macOS. There is **no** user-facing Dock icon selector in Appearance settings. The packaged adaptive icon (`Assets.car` + `CFBundleIconName` = `Icon`) is authoritative when launched as a real `.app`.

On macOS, default development launches a real adaptive `.app`:

```bash
npm run dev                 # packaged adaptive hot development (debug .app + Vite + Rust reload)
npm run dev:raw             # unpackaged tauri dev escape hatch (Classic Dark stand-in; NOT adaptive proof)
npm run dev:bundle-verify   # inspect the prepared debug .app without launching
npm run macos:build-and-run-packaged
npm run macos:run-packaged  # release .app verification
```

Do **not** treat `npm run dev:raw` / bare `tauri dev` as proof of adaptive Icon & Widget Style behavior.

**DORMANT MANUAL SYSTEM:** Classic Dark / Classic Light / Split infrastructure remains in source behind `MANUAL_DOCK_ICON_SELECTION_ENABLED = false` (TypeScript + Rust). UI lives in `ManualDockIconSelector.tsx` and is not mounted while the capability is off. Runtime and IPC normalize manual requests to Follow macOS. Migration `024_reset_dock_icon_follow_macos` resets stored manual prefs. Re-enable later by flipping the flag and mounting the selector — do not rewrite native Dock handling.

| Dormant manual choice | Runtime asset |
| --- | --- |
| Classic Dark | `coreside-dock-dark.png` |
| Classic Light | `coreside-dock-light.png` |
| Split | `coreside-dock-split.png` (composed from canonical Classic Dark + Light marks + shared diagonal mask) |

Commands (still registered; manual authority gated): `commit_dock_icon_preference`, `apply_persisted_dock_icon`. Do not use `set_setting("dockIcon")`.

Adaptive freshness: `npm run brand:verify-adaptive-icon` (fails on missing/stale `Assets.car`).

## Liquid-Glass-inspired treatment

Dock sources use:

- Squircle inset in a transparent canvas (~11% outer margin) so Dock scale matches other macOS apps
- Large mark inside the squircle (trimmed source margins, ~8% inner padding)
- Shared flower placement across Classic Dark, Classic Light, and Split
- Subtle top highlight band
- Thin inner edge ring
- No accent color tinting of the mark
- No blue-purple effects

Readable from 16px through 1024px. Contact sheet: `reports/evidence/branding/dock-icon-contact-sheet.png`.

## Runtime macOS Dock behavior

- Setting: versioned `dockIcon` JSON (defaults / migrates to `follow_macos`)
- While manual selection is dormant, effective authority is always Follow macOS
- Follow macOS in a packaged `.app` (including `npm run dev` debug bundle): `NSApplication.setApplicationIconImage(None)` so `Assets.car` owns the Dock tile
- Follow macOS in unpackaged `npm run dev:raw`: Classic Dark PNG stand-in (not adaptive proof)
- Agent / `set_setting("dockIcon")` cannot change the Dock icon

## Protected branding behavior

Reserved IDs in `src-tauri/src/security/protected_resources.rs` include:

- `core.branding`
- `core.branding.in_app_logo`
- `core.branding.dock_icon`

Any generated change targeting these IDs is rejected before persistence. Logos are not stored as user-editable tool assets.

## Regeneration

```bash
python3 scripts/generate_branding_assets.py
npx tauri icon src-tauri/icons/icon.png --output src-tauri/icons
```

Requires Pillow for the Python script.
