# Release-Candidate Research

**Product:** Coreside  
**Access date:** 2026-08-02 (evening)  
**Public beta:** **NOT READY**  
**Baseline:** [CURRENT_SOURCE_BASELINE.md](./CURRENT_SOURCE_BASELINE.md) · archive `b2f8bf4e…` (Chat AI(5))  
**Active commit:** `90b99538c6c3ce094a7d3b9b09fdc54e7e26e1f6` (may be dirty)

## Purpose

Order remaining work for a **release candidate**, not a public-beta ship claim. Wallpaper compositing closure is **in scope**. Prior Chat AI(4) archive `d8fe0c…` is superseded.

## RC gate map

| Priority | Gate | Notes |
| --- | --- | --- |
| RC1 | Wallpaper visual proof | 0/20/40/60% + None; see [WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md](./WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md) |
| RC2 | AppPaths | Canonical `coreside` / legacy `Coreside` co-location |
| RC3 | Profile boundary | Consumer profile vs protected-core separation evidence |
| RC4 | Full backup/restore | Trusted snapshot path; [BACKUP_AND_RESTORE_RESEARCH.md](./BACKUP_AND_RESTORE_RESEARCH.md) |
| RC5 | Tauri ACL | Manifest + capability `allow-*`; [TAURI_COMMAND_AUTHORITY_RESEARCH.md](./TAURI_COMMAND_AUTHORITY_RESEARCH.md) |
| RC6 | Agent trust Rust wiring | Policy/permissions/recovery choke points in Rust |
| RC7 | E2E | Critical journeys asserted or waived in writing |
| RC8 | Packaged smoke | macOS bundle + cold-start checklist |
| RC9 | Assurance | Honest `release:evidence`; no `passed_partial` → `passed` inflation |

## Explicit non-claims

- Public beta **NOT READY**.
- Do not invent E2E / packaged-smoke / visual-proof pass without evidence.
- Dirty worktree ≠ release snapshot.

## Plan

Phases: [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) (RC section at top).
