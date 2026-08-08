#!/usr/bin/env bash
# Verify precompiled Assets.car matches current Icon Composer sources.
# Delegates to scripts/verify-adaptive-icon.mjs so fingerprint hashing stays
# identical to brand:compile-adaptive-icon / brand:verify-adaptive-icon.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec node "$ROOT/scripts/verify-adaptive-icon.mjs" "$@"
