#!/usr/bin/env bash
# Build (optional) and launch the packaged macOS .app for adaptive-icon testing.
# Never installs to /Applications. Never uses tauri dev.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_PATH="$ROOT/src-tauri/target/release/bundle/macos/Coreside.app"
SKIP_BUILD=0
if [[ "${1:-}" == "--skip-build" ]]; then
  SKIP_BUILD=1
fi

echo "Adaptive macOS appearance must be tested using this packaged application, not tauri dev."

cd "$ROOT"
node scripts/verify-adaptive-icon.mjs

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  npm run brand:generate
  # Freshness already checked; compile only if fingerprint would fail after generate of dock tiles
  # (adaptive sources unchanged by brand:generate). Build the .app.
  npm run tauri -- build --bundles app
fi

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing packaged app: $APP_PATH" >&2
  echo "Run without --skip-build, or build with: npm run tauri -- build --bundles app" >&2
  exit 1
fi

echo "Exact packaged app under test:"
echo "  $APP_PATH"
plutil -p "$APP_PATH/Contents/Info.plist" | grep -E 'CFBundleIdentifier|CFBundleIconName|CFBundleName' || true
if [[ -f "$APP_PATH/Contents/Resources/Assets.car" ]]; then
  echo "  Assets.car: $(wc -c < "$APP_PATH/Contents/Resources/Assets.car" | tr -d ' ') bytes"
else
  echo "FAIL: packaged Assets.car missing — adaptive Icon & Widget Style cannot work" >&2
  exit 1
fi
if [[ -f "$APP_PATH/Contents/Resources/icon.icns" ]]; then
  echo "  icon.icns: present"
fi

open "$APP_PATH"
echo "Launched $APP_PATH"
