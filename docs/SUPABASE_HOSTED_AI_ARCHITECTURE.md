# Supabase Hosted AI Architecture

**Product:** Coreside  
**Assessed:** 2026-07-19

## Request path

1. User signs in (Supabase Auth email/password).
2. Session JSON stored in OS keyring (`coreside:hosted-auth`).
3. Desktop calls `ai-gateway` with user JWT + publishable `apikey`.
4. Gateway validates JWT, entitlement, rate limit, idempotency.
5. Optimistic entitlement reserve → upstream provider (server secret) → usage ledger.
6. Consumer UI shows **Coreside AI**; upstream provider/model hidden.

## Edge Functions

| Function | Role |
| --- | --- |
| `ai-gateway` | Hosted chat completions + health |
| `search-gateway` | Hosted Exa discovery |

## Tables (migration `20260719120000_hosted_ai_foundation.sql`)

profiles, ai_entitlements, ai_usage_ledger, ai_request_idempotency, hosted_search_usage, hosted_search_cache — RLS enabled; users select own rows only; entitlement writes via service role / Edge Functions.

## Desktop wiring

- `HostedAiProvider` when no BYOK/env key and session present
- `adapter_connected` drives `coreside_hosted` access mode
- Settings: sign-in / sign-out when `VITE_SUPABASE_*` configured

## Still required for live hosted AI

1. Apply migration to the real Supabase project
2. Deploy Edge Functions + set secrets (`CORESIDE_AI_PROVIDER_API_KEY`, model, Exa)
3. Set `VITE_SUPABASE_URL` / `VITE_SUPABASE_PUBLISHABLE_KEY` for the desktop build
4. Provide Supabase `project_id` to MCP tools for remote apply (OAuth alone does not inject project id into this agent session)
