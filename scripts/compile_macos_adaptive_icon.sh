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

run_actool() {
  local workdir="$1"
  xcrun actool "$workdir/Icon.icon" \
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
}

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cp -R "$ICON_SRC" "$WORKDIR/Icon.icon"

set +e
ACTOOL_LOG="$(mktemp)"
run_actool "$WORKDIR" >"$ACTOOL_LOG" 2>&1
ACTOOL_STATUS=$?
set -e

if [[ $ACTOOL_STATUS -ne 0 || ! -f "$OUT_DIR/Assets.car" ]]; then
  # Narrow recovery for the known stale-ibtoold / actool flake (tauri#15315).
  if grep -qiE 'ibtoold|NSPlaceholderArray|nil object|Internal Error|segfault|crash' "$ACTOOL_LOG"; then
    echo "actool failed with known IB tooling symptom; attempting one scoped ibtoold recovery…" >&2
    killall ibtoold 2>/dev/null || true
    sleep 1
    rm -rf "$OUT_DIR"
    mkdir -p "$OUT_DIR"
    set +e
    run_actool "$WORKDIR" >"$ACTOOL_LOG" 2>&1
    ACTOOL_STATUS=$?
    set -e
  fi
fi

if [[ $ACTOOL_STATUS -ne 0 || ! -f "$OUT_DIR/Assets.car" ]]; then
  echo "actool failed to produce Assets.car (exit=$ACTOOL_STATUS)" >&2
  echo "----- actool output -----" >&2
  cat "$ACTOOL_LOG" >&2 || true
  rm -f "$ACTOOL_LOG"
  exit 1
fi
rm -f "$ACTOOL_LOG"

cp "$OUT_DIR/Assets.car" "$DEST_CAR"
# Fingerprint via the same Node algorithm as brand:verify-adaptive-icon.
node "$ROOT/scripts/verify-adaptive-icon.mjs" --write-fingerprint
echo "Wrote $DEST_CAR ($(wc -c < "$DEST_CAR") bytes)"
echo "actool $ACTOOL_VERSION OK"
