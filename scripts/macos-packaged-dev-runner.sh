#!/usr/bin/env bash
# Cargo-shaped runner for `tauri dev --runner` (Tauri CLI 2.11.4).
# Ends in exec so SharedChild PID is Contents/MacOS/Coreside.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREPARE="$ROOT/scripts/macos-packaged-dev-prepare.mjs"
APP="$ROOT/src-tauri/target/debug/bundle/macos/Coreside.app"
EXE="$APP/Contents/MacOS/Coreside"

# Refuse accidental release-path configuration via env override attempts.
if [[ "${CORESIDE_DEV_APP_OVERRIDE:-}" != "" ]]; then
  echo "macos-packaged-dev-runner: CORESIDE_DEV_APP_OVERRIDE is not supported" >&2
  exit 1
fi

SUBCOMMAND="${1:-}"
if [[ -z "$SUBCOMMAND" ]]; then
  echo "macos-packaged-dev-runner: expected cargo-shaped subcommand" >&2
  exit 1
fi
shift

CARGO_ARGS=()
APP_ARGS=()
SEEN_DD=0
for arg in "$@"; do
  if [[ "$SEEN_DD" -eq 1 ]]; then
    APP_ARGS+=("$arg")
  elif [[ "$arg" == "--" ]]; then
    SEEN_DD=1
  else
    CARGO_ARGS+=("$arg")
  fi
done

if [[ "$SUBCOMMAND" != "run" ]]; then
  # Accidental build.runner config — forward to real cargo, never wrap.
  exec cargo "$SUBCOMMAND" "${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"}"
fi

# Build + install + sign via Node (argv arrays; no shell-eval of paths).
node "$PREPARE" "${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"}"

if [[ ! -x "$EXE" ]]; then
  echo "macos-packaged-dev-runner: bundled executable missing: $EXE" >&2
  exit 1
fi

# Become the app process — required for Tauri watch kill/relaunch + adaptive Dock.
exec "$EXE" "${APP_ARGS[@]+"${APP_ARGS[@]}"}"
