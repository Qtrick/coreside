# Release Readiness

**Product:** Coreside v0.1.0  
**Phase:** RC3.6 — hosted AI scaffold + local privacy + multimodal send path  
**Source of truth:** `reports/readiness-ladder.json`, `reports/assurance-report.json` (regenerate below)

Do **not** copy test counts into this document by hand. Prefer summarizing the evidence files.

## RC3.6 checkpoint (2026-08-04)

| Track | Ladder | Public beta | Notes |
| --- | --- | --- | --- |
| Local-first BYOK | **Development Build** | **Not ready** | Dirty tree; Journey 11 **Isolated Desktop Verified**; `e2e-results` **passed_partial** (J7) — not a suite Desktop Verified pass |
| Hosted Coreside AI | **Development Build** | **Not ready** | Gateway + billing scaffold unit-tested; private alpha deliberately deferred |

**Narrow assurance:** `npm run assurance:report` → **passed**, 0 findings (narrow validator; Journey 11 isolated on current fingerprint). **Not** a full release pass — J7 partial blocks suite Desktop Verified.

**Do not claim:** Desktop Verified for journeys 1–14, Packaged Verified, Human Accepted, or public beta readiness.

Regenerate:

```bash
npm run audit:current-source
npm run audit:readiness
npm run assurance:report
# Optional isolated ACL proof (already recorded on current fingerprint):
# npx wdio run e2e/wdio.conf.ts --suite existing-authority  # CORESIDE_E2E_SEED=existing
```

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
| Internal alpha (BYOK + local) | **No (dirty tree)** — re-run on clean commit + fuller E2E |
| Local public beta | **No** — needs full E2E matrix + packaged smoke (see `docs/E2E_EXECUTION.md`) |
| Hosted Coreside AI private alpha | **No** — gateway/billing scaffold only; see `reports/hosted-ai-readiness.json` |
| Consumer launch | **No** until local beta checklist is Complete |

Historical July 2026 `reports/release-gates.json` values are **superseded**. The evidence script marks them historical.

## Related docs

- `docs/E2E_EXECUTION.md` — desktop journeys
- `docs/PACKAGED_BUILD_ASSURANCE.md` — packaging
- `docs/KNOWN_ISSUES.md` — open gaps
- `docs/MANUAL_RELEASE_CHECKLIST.md` — human acceptance
