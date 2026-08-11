# Supabase Hosted AI Architecture

**Product:** Coreside  
**Assessed:** 2026-07-19

## Request path

1. User signs in (Supabase Auth email/password).
2. Session JSON stored in OS keyring (`coreside:hosted-auth`).
3. Desktop calls `ai-gateway` with user JWT + publishable `apikey`.
4. Gateway validates JWT, rate limit, then `reserve_hosted_ai_request` (service_role RPC).
5. Upstream provider (server secret) → `settle_hosted_ai_request` or `fail_hosted_ai_request`.
6. Consumer UI shows **Coreside AI**; upstream provider/model hidden.

**Security:** Upstream provider API keys (`CORESIDE_AI_PROVIDER_API_KEY`, Exa keys) live only in Edge Function secrets and never ship to desktop, SQLite, or the webview. Desktop sends chat messages + user JWT only.

## Portable gateway contract

`supabase/functions/ai-gateway/contract.json` documents the desktop ↔ `ai-gateway` JSON contract (auth headers, POST body, success/error shapes). Keep Rust `HostedAiProvider` aligned with this file.

## Plan catalog and entitlements

Migrations:

- `20260804120000_hosted_ai_free_plan_entitlements.sql` — `free` + `beta` catalog; new users get **Free** (no auto 50-request grant)
- `20260804140000_hosted_ai_billing_reservations.sql` — `personal` / `pro` catalog rows, Stripe billing tables, atomic `reserve_hosted_ai_request` / `settle_hosted_ai_request` / `fail_hosted_ai_request` RPC stubs (service_role only)

`ai_plan_catalog` is read-only for authenticated users. Entitlement and subscription writes happen only via Edge Functions / service role.

## Edge Functions

| Function | Role |
| --- | --- |
| `ai-gateway` | Hosted chat completions + health |
| `search-gateway` | Hosted Linkup fast source discovery |
| `billing-checkout` | Stripe Checkout session (server-bound `client_reference_id` + metadata) |
| `billing-portal` | Stripe Customer Portal session stub |
| `stripe-webhook` | Webhook signature verification, idempotency, entitlement sync |

## Tables (migration `20260719120000_hosted_ai_foundation.sql`)

profiles, ai_entitlements, ai_usage_ledger, ai_request_idempotency, hosted_search_usage, hosted_search_cache — RLS enabled; users select own rows only; entitlement writes via service role / Edge Functions.

## Desktop wiring

- `HostedAiProvider` when no BYOK/env key and session present
- `adapter_connected` drives `coreside_hosted` access mode
- Settings: sign-in / sign-out when `VITE_SUPABASE_*` configured; plan read from `ai_entitlements` when signed in (falls back to contract defaults)

## Still required for live hosted AI

1. Apply migration to the real Supabase project
2. Deploy Edge Functions + set secrets (`CORESIDE_AI_PROVIDER_API_KEY`, model, Exa)
3. Set `VITE_SUPABASE_URL` / `VITE_SUPABASE_PUBLISHABLE_KEY` for the desktop build
4. Provide Supabase `project_id` to MCP tools for remote apply (OAuth alone does not inject project id into this agent session)
