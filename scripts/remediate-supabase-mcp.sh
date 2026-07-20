#!/usr/bin/env bash
# Remediate ~/.cursor/mcp.json: remove embedded Supabase PAT, switch to remote OAuth URL.
# Run manually: bash scripts/remediate-supabase-mcp.sh
# Never prints secret values.

set -euo pipefail

MCP="${HOME}/.cursor/mcp.json"
EXAMPLE="$(cd "$(dirname "$0")/.." && pwd)/.mcp.json.example"

if [[ ! -f "$MCP" ]]; then
  echo "No ~/.cursor/mcp.json found — nothing to remediate."
  exit 0
fi

TS="$(date +%Y%m%d%H%M%S)"
BAK="${HOME}/.cursor/mcp.json.bak.${TS}"
cp "$MCP" "$BAK"
chmod 600 "$BAK" "$MCP"

python3 - "$MCP" "$EXAMPLE" <<'PY'
import json, sys
from pathlib import Path

path = Path(sys.argv[1])
example = Path(sys.argv[2])
raw = json.loads(path.read_text())
servers = raw.get("mcpServers") or {}
agent = servers.get("agentmemory")
ex = json.loads(example.read_text())
ex_servers = ex.get("mcpServers") or {}
if not isinstance(agent, dict):
    agent = ex_servers.get("agentmemory", {})
supabase = ex_servers.get("supabase") or {
    "url": "https://mcp.supabase.com/mcp?read_only=true&features=database,docs,debugging,development,functions"
}
out = {"mcpServers": {"agentmemory": agent, "supabase": supabase}}
path.write_text(json.dumps(out, indent=2) + "\n")
sb = out["mcpServers"]["supabase"]
print("supabase_mode:", "url" if "url" in sb else "command")
print("token_arg_present:", "--token" in json.dumps(sb))
print("sbp_shape_in_active:", "sbp_" in path.read_text())
print("done — rotate the old PAT at https://supabase.com/dashboard/account/tokens")
print("then authenticate Supabase MCP via Cursor OAuth and delete the .bak file")
PY

chmod 600 "$MCP"
echo "Backup: $BAK"
