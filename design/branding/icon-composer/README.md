# Icon Composer packaging

Coreside’s Follow macOS mode is wired for a genuine Icon Composer `.icon`
compiled into `Assets.car` (`actool` 26+, Xcode 26+).

## Tooling status (checked 2026-08-06)

- Xcode 26.6 / `actool` short-bundle-version **26.6** — available on this machine
- Icon Composer / `ictool` 1.6 — available
- Source package: `src-tauri/icons/Coreside.icon`
- Precompiled catalog path: `src-tauri/icons/Assets.car` (listed in `tauri.conf.json` → `bundle.icon`)
- `Info.plist` sets `CFBundleIconName` = `Icon`

Prefer the precompiled `Assets.car` path over relying on Tauri’s in-bundle `actool`
invocation (see tauri#15315 intermittent nil crash).

**Not yet claimed:** a fresh packaged `.app` inspection proving
`Contents/Resources/Assets.car` + Icon & Widget Style cycling. Source wiring alone
is not packaged verification.

## Regenerate

```bash
# After editing Coreside.icon layers / icon.json:
./scripts/compile_macos_adaptive_icon.sh

# Then package:
npm run tauri build -- --bundles app
```

Confirm in the packaged app before calling adaptive packaging ready:

1. `Contents/Resources/Assets.car` exists
2. `Contents/Info.plist` contains `CFBundleIconName` = `Icon`
3. System Settings → Appearance → Icon & Widget Style cycles Default / Dark / Clear / Tinted

Do not claim adaptive packaging without a fresh `Assets.car` in the installed bundle.
