#!/usr/bin/env bash
# Build (optional) and launch the packaged macOS .app for adaptive-icon testing.
# Never installs to /Applications. Never uses tauri dev.
# Verifies the running executable belongs to the expected release .app.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_PATH="$ROOT/src-tauri/target/release/bundle/macos/Coreside.app"
EXE_PATH="$APP_PATH/Contents/MacOS/Coreside"
SKIP_BUILD=0
ALLOW_CONCURRENT=0
REPLACE_RUNNING=0

for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --allow-concurrent) ALLOW_CONCURRENT=1 ;;
    --replace-running) REPLACE_RUNNING=1 ;;
    -h|--help)
      echo "Usage: $0 [--skip-build] [--allow-concurrent] [--replace-running]"
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

echo "Adaptive macOS appearance must be tested using this packaged application."
echo "Exact target: $APP_PATH"

cd "$ROOT"
node scripts/verify-adaptive-icon.mjs

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  npm run brand:generate
  npm run tauri -- build --bundles app
fi

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing packaged app: $APP_PATH" >&2
  echo "Run without --skip-build, or build with: npm run tauri -- build --bundles app" >&2
  exit 1
fi
if [[ ! -x "$EXE_PATH" ]]; then
  echo "Missing packaged executable: $EXE_PATH" >&2
  exit 1
fi

echo "Exact packaged app under test:"
echo "  $APP_PATH"
plutil -p "$APP_PATH/Contents/Info.plist" | grep -E 'CFBundleIdentifier|CFBundleIconName|CFBundleName' || true
if [[ -f "$APP_PATH/Contents/Resources/Assets.car" ]]; then
  CAR_SHA="$(shasum -a 256 "$APP_PATH/Contents/Resources/Assets.car" | awk '{print $1}')"
  SRC_SHA="$(shasum -a 256 "$ROOT/src-tauri/icons/Assets.car" | awk '{print $1}')"
  echo "  Assets.car: $(wc -c < "$APP_PATH/Contents/Resources/Assets.car" | tr -d ' ') bytes"
  echo "  Assets.car sha256: $CAR_SHA"
  if [[ "$CAR_SHA" != "$SRC_SHA" ]]; then
    echo "FAIL: packaged Assets.car SHA != source catalog" >&2
    exit 1
  fi
else
  echo "FAIL: packaged Assets.car missing — adaptive Icon & Widget Style cannot work" >&2
  exit 1
fi
if [[ -f "$APP_PATH/Contents/Resources/icon.icns" ]]; then
  echo "  icon.icns: present"
fi

# Enumerate Coreside Mach-O processes by argv[0] only (not path substrings).
# No broad killall.
coreside_ps() {
  # $1 = conflict | same
  local mode="$1"
  ps -axo pid=,args= | awk -v exe="$EXE_PATH" -v mode="$mode" '
    {
      pid=$1
      $1=""
      sub(/^ /,"")
      cmd=$0
      if (cmd ~ /macos-run-packaged|ps -axo/) next
      n=split(cmd, a, " ")
      if (n < 1) next
      bin=a[1]
      # Real app/binary only — ignore editors/npm whose args mention Coreside.
      if (bin !~ /\/Coreside$/) next
      if (mode == "conflict" && bin != exe) print pid "\t" cmd
      if (mode == "same" && bin == exe) print pid
    }
  ' || true
}

CONFLICTS="$(coreside_ps conflict)"
SAME="$(coreside_ps same)"

if [[ -n "$CONFLICTS" && "$ALLOW_CONCURRENT" -eq 0 ]]; then
  echo "Other Coreside processes detected:"
  echo "$CONFLICTS" | sed 's/^/  /'
  echo "FAIL: refusing to activate an ambiguous Coreside instance." >&2
  echo "Quit the other process, or pass --allow-concurrent." >&2
  echo "(--replace-running only restarts this release app; it does not clear other bundles.)" >&2
  exit 1
fi

if [[ "$REPLACE_RUNNING" -eq 1 && -n "$SAME" ]]; then
  echo "--replace-running: terminating processes whose executable is this release app"
  while read -r pid; do
    [[ -n "$pid" ]] || continue
    kill "$pid" 2>/dev/null || true
  done <<< "$SAME"
  sleep 0.5
fi

# Force a new instance of THIS bundle (avoid activating a different Coreside).
open -n "$APP_PATH"

FOUND_PID=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
  FOUND_PID="$(coreside_ps same | awk 'NR==1 { print $1; exit }')"
  if [[ -n "$FOUND_PID" ]]; then
    break
  fi
  sleep 0.3
done

if [[ -z "$FOUND_PID" ]]; then
  echo "FAIL: could not verify a running process for $EXE_PATH" >&2
  exit 1
fi

echo "Launched exact packaged executable:"
echo "  PID: $FOUND_PID"
echo "  Executable: $EXE_PATH"
echo "  App: $APP_PATH"
