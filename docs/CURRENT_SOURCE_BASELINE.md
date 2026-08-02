# Current Source Baseline

**Product:** Coreside  
**Access date:** 2026-08-02 (evening)  
**Phase:** Release-candidate research / closure  
**Public beta:** **NOT READY**  
**Reports:** `reports/current-source-baseline.json` · `reports/active-versus-uploaded-coreside.json`

## Active tree

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `90b99538c6c3ce094a7d3b9b09fdc54e7e26e1f6` |
| Dirty | **Yes** — may include wallpaper/bootstrap review fixes after HEAD |

## Uploaded archive (authoritative)

| Field | Value |
| --- | --- |
| Observed names | `Coreside Chat AI.zip` / Chat AI(5) |
| Path observed | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| SHA-256 | `b2f8bf4e244cd82976de39cacb484cf86c641ba08314c285f7467e115dbf17f2` |
| Approx bytes | 2824600 |
| Supersedes | Prior archive `d8fe0c3110e8fd9809962d61bb05a594b6cf821c109ce0d4c305aeec3152b36b` (Chat AI(4)) — **do not use** |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `f32e3db1ed083c4c69903b536cb15edbb1ba616e8f8affd75c113cd7004e3be6` |
| `src-tauri/Cargo.lock` | `4b118413c6104abc1eab2df0455e2d2218df058c59c075866191c677709d0729` |

## Counts (archive ↔ active)

| Tree | `src` files | Rust `.rs` | Migrations `.sql` | E2E specs |
| --- | --- | --- | --- | --- |
| Archive | 148 | 170 | 15 | 10 |
| Active (dirty) | 149 | 172 | 15 | 10 |

## Relationship

Lockfiles and archive SHA-256 match Chat AI(5). Active HEAD is `90b99538…` with a dirty worktree carrying RC closure work (AppPaths, `.coreside-backup`, agent trust wiring, shell permission removal, wallpaper visual model). Do **not** treat a dirty tree as a release snapshot.

## Scope

| Area | Status |
| --- | --- |
| Wallpaper compositing closure | **IN SCOPE** — unit visual model passed; desktop/packaged proof pending |
| Full backup archive | **Partial** — `.coreside-backup` DB snapshot + preview; media/attachments + full restore swap pending |
| Public beta ship | **NOT READY** |

## Related research

- [RELEASE_CANDIDATE_RESEARCH.md](./RELEASE_CANDIDATE_RESEARCH.md)
- [WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md](./WALLPAPER_COMPOSITING_CLOSURE_RESEARCH.md)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) (RC phases)
