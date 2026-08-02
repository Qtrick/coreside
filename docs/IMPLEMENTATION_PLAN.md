# Implementation Plan — Public Beta Overhaul (current phase)

**Product:** Coreside  
**Status:** Docs / research foundation; **public beta NOT READY**  
**Last updated:** 2026-08-01  
**Access date:** 2026-08-01

### Scope note (2026-08-01)

- **Wallpaper implementation is OUT OF SCOPE for this phase.** Do not change wallpaper compositing/runtime code while executing this plan’s Settings/security/docs track. Residual wallpaper risks stay tracked in existing wallpaper docs/reports only.
- **Settings IA is the priority** — consumer category navigation and language per [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md) and [CONSUMER_SETTINGS_RESEARCH.md](./CONSUMER_SETTINGS_RESEARCH.md).
- Inventories / journeys / assurance stubs: [FEATURE_INVENTORY.md](./FEATURE_INVENTORY.md) · [USER_JOURNEY_MATRIX.md](./USER_JOURNEY_MATRIX.md) · [RELEASE_ASSURANCE_RESEARCH.md](./RELEASE_ASSURANCE_RESEARCH.md) · [DATABASE_READINESS_RESEARCH.md](./DATABASE_READINESS_RESEARCH.md) · [VENDO_DEEP_ADOPTION_AUDIT.md](./VENDO_DEEP_ADOPTION_AUDIT.md).

Research: [BETA_OVERHAUL_RESEARCH.md](./BETA_OVERHAUL_RESEARCH.md) · [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md) · [SECURITY_VERIFICATION_STANDARD.md](./SECURITY_VERIFICATION_STANDARD.md) · [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)  
Reports: `reports/feature-inventory.json` · `reports/user-journey-matrix.json` · `reports/coreside-uploaded-manifest.json` · `reports/vendo-uploaded-manifest.json` · `reports/local-beta-readiness.json`

## Public-beta phase goals

1. **Settings IA** — General → About (+ Added Settings); no developer jargon in primary labels.
2. ASVS L2–style security verification evidence — **no false certification claims**.
3. Agent Allow / RequireApproval / Block choke points documented and test-backed.
4. Honest feature + journey inventories; public beta remains **NOT READY** until packaged evidence lands.
5. Wallpaper / surface compositing — **deferred** (see historical section below); no wallpaper code changes in this phase.

## Public-beta phases

| Phase | Scope | Exit criteria |
| --- | --- | --- |
| 0 | Docs + archive hashes + inventories | This section + research docs + JSON reports present |
| 1 | Settings IA navigation (consumer categories) | Primary nav matches proposed IA; Wallpapers stay Added → Templates |
| 2 | Security verification evidence refresh | Assurance doc + findings JSON honest; open P0s fixed or waived |
| 3 | Agent security regression suite | Policy/gateway/injection boundary tests green |
| 4 | Beta readiness gate | Packaged smoke + desktop journey matrix recorded — **not claimed until run** |

## Explicit non-claims

- Public beta **NOT READY** as of this plan update.
- Do not invent that full E2E or packaged smoke has passed.
- Not ASVS/WCAG certified.
- Wallpaper visual proof is **not** an exit criterion for this phase.

## Rollback

Feature-flag settings nav independently. Prefer solid overlays if later compositing work regresses readability. Never leave schema wallpaper store accepting legacy `{kind:"none"}`.

---

# Implementation Plan — Coreside Beta Overhaul (historical; wallpaper track deferred)

**Note:** Wallpaper nested-opacity / `--surface` migration goals below are **deferred** relative to the public-beta Settings IA priority above. Keep for continuity; do not treat as active phase work.

Research (wallpaper / compositing): [SURFACE_COMPOSITING_MODEL.md](./SURFACE_COMPOSITING_MODEL.md) · [WALLPAPER_ASSURANCE.md](./WALLPAPER_ASSURANCE.md)  
Reports: `reports/background-rule-inventory.json` · `reports/wallpaper-compositing.json` · `reports/consumer-polish-findings.json`

## Deferred beta overhaul goals (wallpaper)

1. Wallpaper visibly composites through protected chrome (nested opaque `--surface` eliminated or excepted).
2. Normative surface depth / alpha model (0/20/40/60%, no parent opacity).

## Deferred wallpaper phases (reference)

| Phase | Scope | Exit criteria |
| --- | --- | --- |
| W1 | Wallpaper nested-opacity fix + remaining `--surface` migration | Visual proof pack; `wallpaper-compositing.json` verified |

---

# Implementation Plan — Adaptive Viewport, Wallpaper Compositing, and Consumer Polish

**Product:** Coreside  
**Status:** Planning / research complete; implementation not claimed complete  
**Last updated:** 2026-08-01

Research: [ADAPTIVE_LAYOUT_RESEARCH.md](./ADAPTIVE_LAYOUT_RESEARCH.md) · [WINDOW_ORCHESTRATION_RESEARCH.md](./WINDOW_ORCHESTRATION_RESEARCH.md) · [WALLPAPER_COMPOSITING_RESEARCH.md](./WALLPAPER_COMPOSITING_RESEARCH.md) · [CONSUMER_POLISH_RESEARCH.md](./CONSUMER_POLISH_RESEARCH.md)  
Contract: [APPLICATION_VIEWPORT_CONTRACT.md](./APPLICATION_VIEWPORT_CONTRACT.md)  
Audit: [CONSUMER_POLISH_AUDIT.md](./CONSUMER_POLISH_AUDIT.md) · `reports/consumer-polish-findings.json` · `reports/layout-overflow-baseline.json`

## Goals

1. Zero application-level horizontal overflow inside the native client area (Viewport Contract).
2. Adaptive AppShell modes (wide / standard / compact) with container-aware Tool Canvas and headers.
3. Trusted window orchestration (smart expansion, restoration, secondary window sizing) behind Adaptive Window Sizing.
4. Wallpaper compositing: intensity vs interface transparency; clear None path; readability preserved.
5. Consumer polish: solid wordmark text, friendly errors, overflow-safe actions, native-feeling generated tools.

## Phased work

| Phase | Scope | Exit criteria |
| --- | --- | --- |
| A | Docs + overflow baseline skeleton | This plan + research + contract + audit + JSON baselines |
| B | P0 polish fixes (wallpaper clear, tool header, wordmark) | P0s closed in audit JSON; manual repro green |
| C | Root layout hardening + scroll owners + overflow audit command | Nested overflow report; no root horizontal scroll at min size |
| D | Container queries / layout modes / split | Compact mode never clips half a tool |
| E | Interface transparency tokens + layering | Wallpaper visible; readability holds at 0–60% |
| F | Window orchestrator + setting + secondary sizing | Policy-gated expansion; rollback-safe |
| G | Generated tool viewport metadata | Defaults + clamps; contract obeyed |

## Non-goals (this phase)

- Enterprise multiuser / org admin
- Unrestricted generative HTML/JS
- Copying Vendo source or branding
- Full WCAG certification paperwork

## Verify (planned / existing)

```bash
npm run typecheck
npm run lint
npm run test
npm run test:layout-overflow   # add when audit harness lands
npm run doctor
# Desktop: min 900×600, live wallpaper None cycle, tool header at narrow split, reduced motion
```

## Rollback

Feature-flag layout modes, transparency UI, and window orchestration independently. Prefer Off for Adaptive Window Sizing if uncertain. Never leave schema wallpaper store accepting legacy `{kind:"none"}`.

---

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
