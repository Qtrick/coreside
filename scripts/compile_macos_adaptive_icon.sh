#!/usr/bin/env bash
# Precompile Coreside.icon → Assets.car for Tauri packaging.
# Prefer this over relying on Tauri's in-bundle actool invocation (tauri#15315).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SRC="$ROOT/src-tauri/icons/Coreside.icon"
OUT_DIR="$ROOT/src-tauri/icons/adaptive-build"
DEST_CAR="$ROOT/src-tauri/icons/Assets.car"

if [[ ! -d "$ICON_SRC" ]]; then
  echo "Missing $ICON_SRC" >&2
  exit 1
fi

ACTOOL_VERSION="$(xcrun actool --version --output-format=human-readable-text 2>/dev/null | awk '/short-bundle-version:/ {print $2}')"
MAJOR="${ACTOOL_VERSION%%.*}"
if [[ -z "$MAJOR" || "$MAJOR" -lt 26 ]]; then
  echo "actool >= 26 required (got: ${ACTOOL_VERSION:-unavailable})" >&2
  exit 1
fi

# Stale ibtoold can make actool fail intermittently (tauri#15315 commentary).
killall ibtoold 2>/dev/null || true

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

# actool requires the document basename to match --app-icon when copied as Icon.icon
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cp -R "$ICON_SRC" "$WORKDIR/Icon.icon"

xcrun actool "$WORKDIR/Icon.icon" \
  --compile "$OUT_DIR" \
  --output-format human-readable-text \
  --notices \
  --warnings \
  --errors \
  --output-partial-info-plist "$OUT_DIR/assetcatalog_generated_info.plist" \
  --app-icon Icon \
  --include-all-app-icons \
  --enable-on-demand-resources NO \
  --development-region en \
  --target-device mac \
  --minimum-deployment-target 26.0 \
  --platform macosx

if [[ ! -f "$OUT_DIR/Assets.car" ]]; then
  echo "actool did not produce Assets.car" >&2
  exit 1
fi

cp "$OUT_DIR/Assets.car" "$DEST_CAR"
echo "Wrote $DEST_CAR ($(wc -c < "$DEST_CAR") bytes)"
echo "actool $ACTOOL_VERSION OK"
