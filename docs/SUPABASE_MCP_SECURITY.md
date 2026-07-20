# Supabase MCP Security

**Product:** Coreside  
**Assessed:** 2026-07-19  
**Access date for official docs:** 2026-07-19

## Sources

- https://supabase.com/docs/guides/ai-tools/mcp
- Current Cursor MCP config at `~/.cursor/mcp.json` (outside the repository)

## Findings (no secret values printed)

| Check | Result |
| --- | --- |
| Repo-tracked `mcp.json` / `.cursor/mcp.json` | **Absent** — not in Git |
| `.gitignore` entries | `mcp.json`, `.mcp.json`, `.cursor/mcp.json` |
| Active Cursor config | Remote OAuth URL: `https://mcp.supabase.com/mcp?read_only=true&features=database,docs,debugging,development,functions` |
| Literal `--token` / `sbp_` in active config | **None** in active file |
| Runtime dependency on MCP | **None** — desktop uses Edge Functions + Auth only |

## Prior exposure

A development personal access token may have been present earlier in local Cursor config (user-reported). Remediation:

1. Run `bash scripts/remediate-supabase-mcp.sh` (already switches to OAuth URL template).
2. **Rotate** any previously exposed PAT at https://supabase.com/dashboard/account/tokens (user action — this session cannot claim rotation completed).
3. Re-authenticate Supabase MCP via Cursor OAuth.
4. Delete `~/.cursor/mcp.json.bak.*` after confirming OAuth works.
5. Prefer **read-only** + scoped features (as in `.mcp.json.example`).

## Safe example

See `.mcp.json.example` in the repository (placeholders / OAuth URL only).

## Environment policy

- MCP → **development / staging inspection only**
- Never production user data via MCP when avoidable
- Never embed MCP tokens in Tauri builds, Edge Functions, or SQLite
