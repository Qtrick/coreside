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

### Dock / application icons (follow OS appearance, not in-app theme)

| OS appearance | Asset | Content |
| --- | --- | --- |
| Light | `coreside-dock-dark.png` | Dark squircle + white mark |
| Dark | `coreside-dock-light.png` | Light squircle + black mark |

Packaged copies live under `src-tauri/resources/branding/`.

Default packaged Tauri icons (`src-tauri/icons/*`, including `icon.icns` / `icon.ico`) are generated from the dark dock variant via `tauri icon`.

## Liquid-Glass-inspired treatment

Dock sources use:

- Squircle inset in a transparent canvas (~11% outer margin) so Dock scale matches other macOS apps
- Large mark inside the squircle (trimmed source margins, ~8% inner padding)
- Subtle top highlight band
- Thin inner edge ring
- No accent color tinting of the mark
- No blue-purple effects

Readable from 32px through 1024px.

## Runtime macOS Dock switching

- Setting: `dockIcon` — `auto` | `dark` | `light` (Base Settings → Appearance)
- Command: `set_dock_icon(preference, osIsDark)`
- `auto` listens to `prefers-color-scheme` (OS), **not** the in-app Appearance override
- `dark` / `light` lock the corresponding brand tile
- Implementation: `objc2` / AppKit `NSApplication::setApplicationIconImage` on macOS; no-op elsewhere
- Limitation: affects the running Dock tile; Finder’s permanent app icon remains the packaged default

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
