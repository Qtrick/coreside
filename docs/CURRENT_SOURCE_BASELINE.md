# Current Source Baseline

**Product:** Coreside  
**Access date:** 2026-08-02  
**Public beta:** **NOT READY**  
**Reports:** `reports/current-source-baseline.json` · `reports/active-versus-uploaded-coreside.json`

## Active tree

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `57f80bcb74de4107760350ad887df5d4ea246a3d` |
| Dirty | **Yes** — Settings IA + review fixes uncommitted |

## Uploaded archive

| Field | Value |
| --- | --- |
| Observed names | `Coreside Chat AI.zip` / `Chat AI.zip` |
| Path observed | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| SHA-256 | `d8fe0c3110e8fd9809962d61bb05a594b6cf821c109ce0d4c305aeec3152b36b` |
| Note | Matches Chat AI(4) hash from task |
| Extract | `.reference/coreside-uploaded-2026-08-02/coreside-main` |
| Extract commit | Same as active HEAD |

## Lockfiles (active = extract)

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `f32e3db1ed083c4c69903b536cb15edbb1ba616e8f8affd75c113cd7004e3be6` |
| `src-tauri/Cargo.lock` | `4b118413c6104abc1eab2df0455e2d2218df058c59c075866191c677709d0729` |

## Counts (approx match)

| Tree | `src` files | Rust `.rs` | Migrations `.sql` | E2E specs |
| --- | --- | --- | --- | --- |
| Task approx | 144 | 165 | 15 | 10 |
| Extract | 144 | 166 | 15 | 10 |
| Active | 144 | 179 | 15 | 10 |

## Relationship

Archive ≈ `coreside-main` snapshot at the shared commit. Active may be ahead with Settings IA + review fixes (dirty). Do **not** treat the dirty worktree as a release snapshot.

## Out of scope

Wallpaper redesign / wallpaper runtime code — do not change while closing these blockers.

## Open P0 (bootstrap)

`Database::open_default()` failure still **panics** in `src-tauri/src/lib.rs` — hard crash instead of recoverable startup UX.

## Review fixes already present (uncommitted)

1. `scripts/release-evidence.mjs` — `passed_partial` no longer maps to `passed`
2. `package.json` — `assurance:report` → `npm run release:evidence:quick` (not a placeholder)
3. `product_data_dir` — canonical `coreside`, reuse legacy `Coreside` when present

## Related research

- [BETA_BLOCKER_CLOSURE_RESEARCH.md](./BETA_BLOCKER_CLOSURE_RESEARCH.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) (blocker-closure phases)
