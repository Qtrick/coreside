# Implementation Plan — RC3.1 Secure Partial Update Runtime + Public-Beta Blockers

**Product:** Coreside  
**Status:** RC3.1 dependency-order execution in progress; **public beta NOT READY** · **local-first NOT READY** · **Hosted AI NOT READY**  
**Last updated:** 2026-08-03  
**Access date:** 2026-08-03  
**Coreside archive:** `75946b05d7778368700c8d827007d83f1cce0006fcedad8cf69b35f37bb23d69` (`Coreside Chat AI.zip`)  
**Previous Coreside archive:** `ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228`  
**Partial Update archive:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` (byte-identical to prior)  
**Active commit:** `c520fcfba6acdeed7a14ee1e24dd9ea31f67b4c7` (dirty)  

### Scope note (2026-08-03 evening — RC3.1)

Production implementation is underway (not audit-only). Honest landed work this session:

| Workstream | Status |
| --- | --- |
| Baseline vs `75946b05…` | **Done** (dirty tree labeled) |
| Misleading npm script names | **Done** (`preservation-suite` / `transactions`; removed false parity/replay/inspector) |
| True streaming vertical slice | **Landed** — `chat_with_auto` → `chat_stream`; OpenAI live SSE; UI TextDelta; unit TS-1; Journey 12 **written, not executed** |
| Bounded NDJSON parser | **Landed** — canonical `StreamEvent` path + legacy harvest |
| Queue UI | **Landed** — `ConversationQueue` |
| Branch / snapshot / replay / inspector UI | **Landed** — `ConversationHistory` (replay read-only; paced player incomplete) |
| Structured forms → model | **Landed** — ledger inject + structured message blocks (bounded) |
| Maintenance journal startup | **Landed** — Recovery / safe pre-swap clear / skip scheduler (full restore still open) |
| Anthropic/Gemini live SSE | **Open** |
| Progressive preview transaction | **Open** |
| Attachment crash-consistency suite | **Open** (P1) |
| Desktop E2E full suite / packaged smoke | **Open** |

July Partial Update status claims remain **stale** for completeness. HTML/JS/CDN/iframe forms remain **rejected**.

## RC3.1 phases (dependency order)

| # | Scope | Status |
| ---: | --- | --- |
| 1 | Baseline + evidence correction | **Done** (dirty) |
| 2 | Production true streaming slice | **Partial** — unit pass; E2E not_run |
| 3 | Provider + frame protocol completeness | **Partial** — parser hardened; Anthropic/Gemini open |
| 4 | Progressive preview + preservation path | **Open** |
| 5 | Structured forms/context | **Partial** — production inject landed |
| 6 | Queue / branch / replay / inspector UI | **Partial** — usable panels; paced replay open |
| 7 | Attachment integrity + multimodal | **Partial** — foundations + size cap; crash suite open |
| 8 | Durable profile + restore | **Partial** — journal decisions; coherent restore open |
| 9–13 | Media/backup/packages/authority/wallpaper/E2E/assurance | **Open** |

## Explicit non-claims

- Public beta **NOT READY**. Local-first **NOT READY**. Hosted AI **NOT READY**.
- Do not invent E2E, packaged smoke, wallpaper visual-proof, or full Partial Update parity without evidence.
- Dirty worktree ≠ release snapshot.


# Implementation Plan — Release Candidate (prior RC1–RC9 track; superseded for ordering by RC3 phases 1–13 above)

**Product:** Coreside  
**Status:** Historical RC track; retain for continuity — **prefer RC3 dependency table above**  
**Last updated:** 2026-08-02 (evening)  
**Access date:** 2026-08-02 (evening)  
**Archive (historical):** `b2f8bf4e244cd82976de39cacb484cf86c641ba08314c285f7467e115dbf17f2`  
**Active commit (historical note):** `90b99538c6c3ce094a7d3b9b09fdc54e7e26e1f6`

### Scope note (2026-08-02 evening)

- **Wallpaper compositing closure remains in later RC3 order** (phase 21 in full task list) — visual proof + residual nested-surface risks.
- Baseline: [CURRENT_SOURCE_BASELINE.md](./CURRENT_SOURCE_BASELINE.md) · `reports/current-source-baseline.json` · `reports/active-versus-uploaded-coreside.json`
- Research: [RELEASE_CANDIDATE_RESEARCH.md](./RELEASE_CANDIDATE_RESEARCH.md) · [WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md](./WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md)

## Historical RC phases (RC1–RC9)

| Phase | Scope | Exit criteria | Research |
| --- | --- | --- | --- |
| RC1 | Wallpaper visual proof | Proof at 0/20/40/60% + None; `wallpaper-compositing.json` updated | [WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md](./WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md) |
| RC2 | AppPaths | Canonical `coreside` / legacy `Coreside` co-located; documented | Baseline + product_data_dir |
| RC3 | Profile boundary | Consumer profile vs protected-core separation evidence | RC research |
| RC4 | Full backup/restore | Trusted snapshot path; restore verified or waived in writing | [BACKUP_AND_RESTORE_RESEARCH.md](./BACKUP_AND_RESTORE_RESEARCH.md) |
| RC5 | Tauri ACL | AppManifest + capability `allow-*`; not default allow-all custom IPC | [TAURI_COMMAND_AUTHORITY_RESEARCH.md](./TAURI_COMMAND_AUTHORITY_RESEARCH.md) |
| RC6 | Agent trust Rust wiring | Policy/permissions/recovery choke points wired and test-backed in Rust | [AGENT_TRUST_BOUNDARY_RESEARCH.md](./AGENT_TRUST_BOUNDARY_RESEARCH.md) |
| RC7 | E2E | Critical journeys asserted or waived in writing | [E2E_EXECUTION.md](./E2E_EXECUTION.md) |
| RC8 | Packaged smoke | macOS bundle + scan + cold-start checklist recorded | [PACKAGED_SMOKE_RESEARCH.md](./PACKAGED_SMOKE_RESEARCH.md) |
| RC9 | Assurance | Full `release:evidence`; `passed_partial` never → `passed` | [RELEASE_EVIDENCE_RESEARCH.md](./RELEASE_EVIDENCE_RESEARCH.md) |

---

# Implementation Plan — Public Beta Blocker Closure (prior track; superseded by RC above)

**Product:** Coreside  
**Status:** Superseded by RC phases; retained for continuity  
**Last updated:** 2026-08-02  
**Access date:** 2026-08-02  
**Note:** Older baseline commit `57f80bcb…` / archive `d8fe0c…` — use Chat AI(5) `b2f8bf4e…` + `90b99538…` going forward.

### Historical scope note

- Wallpaper was OUT OF SCOPE on this track; **RC track brings compositing closure IN SCOPE**.
- Research hub (historical): [BETA_BLOCKER_CLOSURE_RESEARCH.md](./BETA_BLOCKER_CLOSURE_RESEARCH.md)

## Historical blocker-closure phases (B0–B9)

| Phase | Scope | Exit criteria | Research |
| --- | --- | --- | --- |
| B0 | Paths — `product_data_dir` | Canonical path; co-located data | → RC2 |
| B1 | Bootstrap — no panic on DB open failure | Recoverable startup | Bootstrap recovery |
| B2 | Backup / restore | Trusted snapshot | → RC4 |
| B3 | Privacy / Data Settings | Honest consumer copy | Settings IA |
| B4 | Tauri command authority | Capability allow-list | → RC5 |
| B5 | CSP | Tighten CSP; evidence | security-findings |
| B6 | Agent trust boundary | Evidence recorded | → RC6 |
| B7 | E2E 07 / 10 | Asserted or waived | → RC7 |
| B8 | Packaged smoke | Checklist recorded | → RC8 |
| B9 | Release evidence | Honest evidence | → RC9 |

---

# Implementation Plan — Public Beta Overhaul (Settings IA track; continue in parallel where safe)

**Product:** Coreside  
**Status:** Docs / research foundation; **public beta NOT READY**  
**Last updated:** 2026-08-01  
**Access date:** 2026-08-01

### Scope note (2026-08-01)

- **Wallpaper compositing closure is owned by RC1** above; Settings/security track should not fork a parallel wallpaper redesign.
- **Settings IA remains important** — consumer category navigation and language per [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md) and [CONSUMER_SETTINGS_RESEARCH.md](./CONSUMER_SETTINGS_RESEARCH.md). Prefer **RC phases** when they conflict.
- Inventories / journeys / assurance stubs: [FEATURE_INVENTORY.md](./FEATURE_INVENTORY.md) · [USER_JOURNEY_MATRIX.md](./USER_JOURNEY_MATRIX.md) · [RELEASE_ASSURANCE_RESEARCH.md](./RELEASE_ASSURANCE_RESEARCH.md) · [DATABASE_READINESS_RESEARCH.md](./DATABASE_READINESS_RESEARCH.md) · [VENDO_DEEP_ADOPTION_AUDIT.md](./VENDO_DEEP_ADOPTION_AUDIT.md).

Research: [BETA_OVERHAUL_RESEARCH.md](./BETA_OVERHAUL_RESEARCH.md) · [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md) · [SECURITY_VERIFICATION_STANDARD.md](./SECURITY_VERIFICATION_STANDARD.md) · [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)  
Reports: `reports/feature-inventory.json` · `reports/user-journey-matrix.json` · `reports/coreside-uploaded-manifest.json` · `reports/vendo-uploaded-manifest.json` · `reports/local-beta-readiness.json`

## Public-beta phase goals (Settings / overhaul)

1. **Settings IA** — General → About (+ Added Settings); no developer jargon in primary labels.
2. ASVS L2–style security verification evidence — **no false certification claims**.
3. Agent Allow / RequireApproval / Block choke points documented and test-backed.
4. Honest feature + journey inventories; public beta remains **NOT READY** until packaged evidence lands.
5. Wallpaper / surface compositing — **deferred** (see historical section below); no wallpaper code changes in this phase.

## Public-beta phases (Settings / overhaul)

| Phase | Scope | Exit criteria |
| --- | --- | --- |
| 0 | Docs + archive hashes + inventories | This section + research docs + JSON reports present |
| 1 | Settings IA navigation (consumer categories) | Primary nav matches proposed IA; Wallpapers stay Added → Templates |
| 2 | Security verification evidence refresh | Assurance doc + findings JSON honest; open P0s fixed or waived |
| 3 | Agent security regression suite | Policy/gateway/injection boundary tests green |
| 4 | Beta readiness gate | Packaged smoke + desktop journey matrix recorded — **not claimed until run** |

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

# Implementation Plan — Final Partial Update Gap Completion (July 2026; status claims STALE)

**Product:** Coreside  
**Status:** Historical foundation track — **Coreside completeness claims superseded by 2026-08-03 re-audit**  
**Last updated:** 2026-07-18 (claims stale as of 2026-08-03)

Prior: Application Kernel + Runtime V2. Research: [PARTIAL_UPDATE_FINAL_GAP_RESEARCH.md](./PARTIAL_UPDATE_FINAL_GAP_RESEARCH.md). Audit: [PARTIAL_UPDATE_FINAL_GAP_AUDIT.md](./PARTIAL_UPDATE_FINAL_GAP_AUDIT.md) (**stale status**).  
Current: [PARTIAL_UPDATE_2026_REAUDIT.md](./PARTIAL_UPDATE_2026_REAUDIT.md).

## Shipped as foundation (do not read as release-complete)

1. Migration `014_continuity_scheduler.sql`
2. Preservation engine + frontend helpers
3. Draft protection + conflict banner
4. Patch Scheduler (deps, priority, backpressure, supersession) wired into agent apply path
5. Generated route state + AppRouteShell + same-route no-op
6. Customize mode → same operation protocol + provenance
7. Optimistic local controls with rollback
8. Context ledger **storage** (model-only) — **not** injected into prompts as of 2026-08-03
9. Provider conformance seeded profiles
10. Surface state hydration + continuity suspension
11. Safe parity demo documentation

## Explicitly partial / deferred / corrected by re-audit

- True provider streaming **ABSENT** (July progressive-stream claims stale)
- Forms ledger → prompt injection missing
- Branch / replay / inspector / queue UI largely disconnected
- Full caret/selection fidelity across every control type
- View-transition presets beyond CSS reduced-motion awareness
- Local provider benchmark numbers (no fabricated metrics)
- Public multiuser collaboration (rejected/deferred)
- Unrestricted HTML/JS/CDN/iframe forms (rejected)

## Verify

```bash
npm run audit:partial-update-final-gaps
npm run test:runtime-v2
npm run test:preservation
npm run test:patch-scheduler
npm run typecheck
npm run test
```
