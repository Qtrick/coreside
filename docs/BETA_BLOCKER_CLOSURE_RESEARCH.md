# Beta Blocker Closure Research

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Baseline:** [CURRENT_SOURCE_BASELINE.md](./CURRENT_SOURCE_BASELINE.md)

## Purpose

Close the remaining **public-beta blockers** with documented research and ordered phases. This track does **not** redesign wallpapers.

## Archive continuity

| Item | Value |
| --- | --- |
| Archive SHA-256 | `d8fe0c3110e8fd9809962d61bb05a594b6cf821c109ce0d4c305aeec3152b36b` |
| Extract | `.reference/coreside-uploaded-2026-08-02/coreside-main` |
| Active commit | `57f80bcb74de4107760350ad887df5d4ea246a3d` (dirty review fixes) |

## Blocker map

| Priority | Blocker | Research | Status |
| --- | --- | --- | --- |
| P0 | `Database::open_default` panic on startup failure | This doc + paths/bootstrap work | **Open** |
| P0–P1 | App-data path clarity (`coreside` vs legacy `Coreside`) | Review fix present uncommitted | **Partial** |
| P1 | Full DB backup/restore story | [BACKUP_AND_RESTORE_RESEARCH.md](./BACKUP_AND_RESTORE_RESEARCH.md) | Research done |
| P1 | Privacy / Data Settings IA copy + export journeys | Settings IA track | In progress (dirty) |
| P1 | Tauri custom-command authority (default allow-all) | [TAURI_COMMAND_AUTHORITY_RESEARCH.md](./TAURI_COMMAND_AUTHORITY_RESEARCH.md) | Research done |
| P1 | CSP tightening (`img-src https:` etc.) | Security findings + this track | Open |
| P1 | Agent trust-boundary regression evidence | [AGENT_TRUST_BOUNDARY_RESEARCH.md](./AGENT_TRUST_BOUNDARY_RESEARCH.md) | Research done |
| P1 | E2E journeys 07 / 10 full coverage | [E2E_EXECUTION.md](./E2E_EXECUTION.md) | Partial by design today |
| P1 | Packaged smoke recorded | [PACKAGED_SMOKE_RESEARCH.md](./PACKAGED_SMOKE_RESEARCH.md) | Not claimed |
| P1 | Honest release evidence | [RELEASE_EVIDENCE_RESEARCH.md](./RELEASE_EVIDENCE_RESEARCH.md) | Review fix present |

## Already fixed in dirty tree (do not regress)

1. **Release evidence honesty** — `passed_partial` stays `passed_partial`, never inflates to `passed`.
2. **`assurance:report`** — runs `release:evidence:quick`, not a console placeholder.
3. **`product_data_dir` casing** — prefer `coreside`; reuse existing `Coreside` so media/attachments/db stay co-located.

## Explicit non-goals

- Wallpaper redesign, nested-opacity migration, visual wallpaper proof packs
- Enterprise org/SSO/admin
- ASVS / WCAG certification claims
- Claiming E2E or packaged smoke passed without recorded artifacts

## Ordered closure phases

See top of [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md): paths → bootstrap → backup → privacy/data → command authority → CSP → agent trust → E2E 07/10 → packaged smoke → evidence.

## Verdict

Public beta remains **blocked**. Docs/research for this closure track are in place; implementation must follow the phase order without wallpaper work.
