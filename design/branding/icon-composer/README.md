# Icon Composer packaging (blocked without Apple tooling)

Coreside’s Follow macOS mode requires a genuine Icon Composer `.icon` compiled into
`Assets.car` by Tauri 2.11’s bundler (`actool`, Xcode 26+).

## Current machine gap

- Active developer directory is Command Line Tools only
- `xcodebuild` unavailable
- `actool` unavailable
- Icon Composer.app not installed

## After installing Xcode 26+ and Icon Composer

1. Open Icon Composer and create `src-tauri/icons/Coreside.icon`
2. Add Default, Dark, and Mono appearances using the mark layers in this folder
   (keep Classic identity as the packaged primary; Split remains a manual override)
3. Add `icons/Coreside.icon` to `tauri.conf.json` → `bundle.icon` alongside `icon.icns`
4. Run `npm run tauri build -- --bundles app`
5. Confirm `Contents/Resources/Assets.car` and `CFBundleIconName` in Info.plist

Do not invent a fake `.icon` file. Do not claim adaptive packaging without Assets.car.
