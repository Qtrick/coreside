# Adaptive Icon Research — 2026-08-06

## Provenance

| Artifact | Expected SHA-256 | Actual | Match |
| --- | --- | --- | --- |
| `Coreside Chat AI.zip` | `8d455783…ef3155` | `8d455783…ef3155` | yes |
| `Partial Update Main.zip` | `8666c226…3eb607` | `8666c226…3eb607` | yes |

- Branch: `main`
- Commit: `f648117ef10600794723ae1c00c16473667ec0cb`
- Dirty (ignored): `FFATU/` only — left untouched

## Tooling (live)

| Tool | Version |
| --- | --- |
| Xcode | 26.6 (17F113) |
| `actool` short-bundle-version | **26.6** (bundle 24765) — available |
| Icon Composer / `ictool` | 1.6 (bundle 99.1) |
| `@tauri-apps/cli` | 2.11.4 |
| `@tauri-apps/api` | 2.11.1 |

Stale docs claiming `actool` unavailable are incorrect on this machine.

## Apple findings

1. Packaged adaptive icons are Icon Composer `.icon` → `actool` → `Assets.car` + `CFBundleIconName`.
2. Appearances: Default, Dark, Clear Light/Dark, Tinted Light/Dark. Composer annotations are Default / Dark / Mono; Clear and Tinted are system-derived from Mono.
3. `NSApplication.applicationIconImage` only overrides the **running** Dock tile. Finder / Applications / Launchpad / closed app use the packaged icon.
4. Do not mutate the signed app bundle at runtime.

## Tauri findings

1. CLI 2.11.x supports `.icon` and precompiled `Assets.car` in `bundle.icon`.
2. Bundler copies `.icon` as a directory and runs `actool` with `--app-icon Icon`.
3. Open issue [tauri#15315](https://github.com/tauri-apps/tauri/issues/15315): intermittent `actool` nil crash when Tauri invokes `actool`. **Workaround (officially discussed):** precompile `Assets.car` and list it in `bundle.icon` alongside static PNG/ICNS.
4. No Tauri upgrade required for this pass — precompiled `Assets.car` is the supported route.

## Root causes in current tree (pre-fix)

1. **No adaptive package:** `tauri.conf.json` had only static PNG/ICNS; no `.icon` / `Assets.car`; `Info.plist` had no `CFBundleIconName`. *(Source wiring landed; packaged `.app` confirmation still pending.)*
2. **Split geometry:** `make_split_dock_icon` padded content inside the squircle without filling the tile first, so the opaque region and flower read smaller than Classic (`make_dock_icon`).
3. **Stale readiness text:** branding docs said adaptive packaging is blocked without `actool` even after tooling became available.

## Implementation decisions

1. Unify Split generation onto Classic outer margin, squircle, highlight, ring, and fill-then-mask pipeline so optical size matches.
2. Author `src-tauri/icons/Coreside.icon` (Icon Composer package) from Classic mark layers with Default/Dark/Mono fills.
3. Precompile `src-tauri/icons/Assets.car` via `actool` and reference it from `bundle.icon` (avoid Tauri’s in-bundle `actool` crash path).
4. Keep static `icon.icns` as back-deployment fallback.
5. Follow macOS continues to clear `applicationIconImage`; manual Classic/Split remain temporary AppKit overrides only.
6. Do not upgrade Tauri in this pass.

## Evidence categories used

- Source inspection
- Automated verification (geometry tests, unit tests, tooling version checks)

## Evidence categories still required

- Packaged verification (fresh `tauri build --bundles app` + `Assets.car` / plist inspection)
- Human visual acceptance (Dock / Finder / Icon & Widget Style)
