# Secret Inventory

**Product:** Coreside  
**Assessed:** 2026-07-19  
**Never print secret values.**

| Category | Examples | Storage | Desktop? | Public tables? |
| --- | --- | --- | --- | --- |
| Dev management | Supabase MCP OAuth / PAT | Cursor OAuth or local ignored config | No | No |
| Supabase public | Project URL, publishable key | Build env `VITE_SUPABASE_*` | Yes (publishable only) | N/A |
| Supabase privileged | Service role / secret keys | Edge Function env only | **Never** | **Never** |
| Hosted model keys | OpenRouter / OpenAI / etc. | Edge Function secrets `CORESIDE_AI_PROVIDER_API_KEY` | **Never** | **Never** |
| Hosted Exa | `EXA_API_KEY` on gateway | Edge Function secrets | **Never** (hosted path) | **Never** |
| User BYOK | Provider API keys | OS keyring via Rust | Local only | No |
| Hosted Auth session | Access + refresh JWT | OS keyring `coreside:hosted-auth` | Encrypted OS store | No |
| Local Exa (BYOK search) | Exa key | OS keyring / `.env` (dev) | Local | No |

## Explicit non-storage

Do not place provider keys, service-role keys, MCP tokens, or Exa hosted keys in:

- SQLite
- localStorage
- Exports / diagnostics / Action Log
- Generated tools
- Git
