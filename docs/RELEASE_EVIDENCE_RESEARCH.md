# Release Evidence Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Script:** `scripts/release-evidence.mjs` · commands `release:evidence`, `release:evidence:quick`

## Purpose

Release evidence must be **honest**: it records what actually ran. It must never invent passes or promote partial coverage to a full pass.

## Review fix (present, uncommitted)

| Issue | Before | After (dirty tree) |
| --- | --- | --- |
| E2E `passed_partial` | Mapped into gate status `passed` | Preserved as `passed_partial`; tallied separately |
| `assurance:report` | Placeholder `console.log('See docs/RELEASE_READINESS.md')` | `npm run release:evidence:quick` |

Do not regress these when landing blocker-closure commits.

## Gate semantics

| Status | Meaning |
| --- | --- |
| `passed` | Gate command exited 0 and represents full intended coverage |
| `passed_partial` | Ran successfully but coverage intentionally incomplete (e.g. E2E 07/10) |
| `failed` / `blocked` | Must fail the beta claim |
| `manual_verification_required` | Not run here; human/packaged step still required |
| `not_run` | Skipped in this mode |

Quick mode (`assurance:report` / `release:evidence:quick`) only runs light gates (doctor + package.json parse). It is a **partial** offline signal — verdict style `quick_partial_full_suite_not_run` — not a public-beta green light.

## Artifacts

| File | Role |
| --- | --- |
| `reports/release-evidence.json` | Gate results from the script |
| `reports/e2e-results.json` | Desktop E2E journeys (may be `passed_partial`) |
| `reports/packaging-results.json` | Bundle scan / prior build evidence |
| `reports/local-beta-readiness.json` | Derived readiness stub (not a beta claim) |

## E2E 07 / 10 honesty

`e2e/run.mjs` intentionally marks journeys **7** (multi-window approval race) and **10** (secondary window close) as `passed_partial` even when the suite exits 0. Release evidence must surface that as partial — never as a full desktop-e2e pass.

## Non-claims

- Quick evidence ≠ full evidence.
- Partial E2E ≠ desktop gate passed.
- Public beta **NOT READY** until full evidence mode is green on required gates **and** remaining blockers (bootstrap panic, command ACL, packaged smoke, etc.) are closed or waived in writing.

## Related

- [RELEASE_ASSURANCE_RESEARCH.md](./RELEASE_ASSURANCE_RESEARCH.md)
- [PACKAGED_SMOKE_RESEARCH.md](./PACKAGED_SMOKE_RESEARCH.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
