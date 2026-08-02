# Release Readiness

**Product:** Coreside v0.1.0  
**Source of truth:** `reports/release-evidence.json` (generate with `npm run release:evidence`)

Do **not** copy test counts into this document by hand. Prefer summarizing the evidence file.

## Tracks (kept separate)

| Track | Meaning | Verdict source |
| --- | --- | --- |
| Local-first BYOK | Local app, user keys, tools, projects, search | `reports/local-beta-readiness.json` |
| Hosted Coreside AI | Supabase auth, remote models, Edge Functions | `reports/hosted-ai-readiness.json` |

Hosted AI P0 blockers must **not** fail the local BYOK beta verdict.

## How to regenerate evidence

```bash
npm run release:evidence          # full offline gates
npm run release:evidence:quick    # doctor + inventories only
npm run e2e:desktop               # GUI desktop journeys (merge into e2e-results)
npm run build && npm run package:scan
```

## Current orientation (manual narrative)

| Stage | Eligible? |
| --- | --- |
| Internal alpha (BYOK + local) | **Yes** when offline gates in release-evidence are green |
| Local public beta | **Needs** packaged smoke + honest E2E matrix (see `docs/E2E_EXECUTION.md`) |
| Hosted Coreside AI private alpha | **No** — see hosted-ai-readiness |
| Consumer launch | **No** until local beta checklist is Complete |

Historical July 2026 `reports/release-gates.json` values are **superseded**. The evidence script marks them historical.

## Related docs

- `docs/E2E_EXECUTION.md` — desktop journeys
- `docs/PACKAGED_BUILD_ASSURANCE.md` — packaging
- `docs/KNOWN_ISSUES.md` — open gaps
- `docs/MANUAL_RELEASE_CHECKLIST.md` — human acceptance
