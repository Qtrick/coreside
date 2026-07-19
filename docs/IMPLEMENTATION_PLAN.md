# Implementation Plan — Final Partial Update Gap Completion

**Product:** Coreside  
**Status:** Continuity / scheduler / preservation phase shipped (foundation)  
**Last updated:** 2026-07-18

Prior: Application Kernel + Runtime V2. Research: [PARTIAL_UPDATE_FINAL_GAP_RESEARCH.md](./PARTIAL_UPDATE_FINAL_GAP_RESEARCH.md). Audit: [PARTIAL_UPDATE_FINAL_GAP_AUDIT.md](./PARTIAL_UPDATE_FINAL_GAP_AUDIT.md).

## Completed this phase

1. Migration `014_continuity_scheduler.sql`
2. Preservation engine + frontend helpers
3. Draft protection + conflict banner
4. Patch Scheduler (deps, priority, backpressure, supersession) wired into agent apply path
5. Generated route state + AppRouteShell + same-route no-op
6. Customize mode → same operation protocol + provenance
7. Optimistic local controls with rollback
8. Context ledger (model-only)
9. Provider conformance seeded profiles
10. Surface state hydration + continuity suspension
11. Safe parity demo documentation

## Explicitly partial / deferred

- Full caret/selection fidelity across every control type
- View-transition presets beyond CSS reduced-motion awareness
- Local provider benchmark numbers (no fabricated metrics)
- Public multiuser collaboration (rejected/deferred)
- Unrestricted HTML/JS (rejected)

## Verify

```bash
npm run audit:partial-update-final-gaps
npm run test:runtime-v2
npm run test:preservation
npm run test:patch-scheduler
npm run typecheck
npm run test
```
