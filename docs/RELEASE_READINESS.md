# Release Readiness — Hosted AI + TypeScript 6 phase

**Product:** Coreside v0.1.0  
**Assessed:** 2026-08-01 (registered-action runtime phase)

## Verdict

| Stage | Eligible? |
| --- | --- |
| Internal alpha (BYOK + local) | **Yes** — includes generated-app runtime gates |
| Hosted Coreside AI private alpha | **No** — remote migration/deploy/secrets + project_id not completed |
| Public beta / consumer launch | **No** |

## New gates (2026-08-01)

| Gate | Command | Notes |
| --- | --- | --- |
| Doctor | `npm run doctor` / `--json` | Offline invariants for migrations, registry, gateway, UI wiring |
| CI | `.github/workflows/ci.yml` | typecheck, lint, vitest, doctor, rust check/test, web build |
| Registered actions | `npm run test:registered-actions` | Gateway/policy/grants/approvals |

## Automated evidence this session

| Gate | Result |
| --- | --- |
| TypeScript | **6.0.3** declared + resolved; typecheck exit 0 (~5.1s wall) |
| Vitest | **71/71** pass |
| Lint | Pass (5 warnings) |
| `cargo check` | Pass |
| `cargo test --lib` | Run after this write |
| Hosted E2E against live Supabase | **Not run** |
| Packaged build | **Not run** |
| Remote migration applied | **Not confirmed** (MCP tools require `project_id`) |

## P0 open (hosted launch)

1. Apply `supabase/migrations/20260719120000_hosted_ai_foundation.sql` to the target project
2. Deploy `ai-gateway` + `search-gateway` + Edge secrets
3. Provide build-time `VITE_SUPABASE_*` (and matching Rust env)
4. User rotates any previously exposed Supabase PAT if applicable
5. Live signed-in hosted chat smoke test

## Shipped in tree

- MCP OAuth example + ignore + remediation script
- Hosted AI Edge Function source + search gateway
- Schema + RLS SQL + pgTAP sketch
- Rust `HostedAiProvider` + keyring session + Settings sign-in UI
- Access-mode disclosure for non-BYOK
- TypeScript 6.0.3 migration
