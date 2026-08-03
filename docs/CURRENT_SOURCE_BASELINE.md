# Current Source Baseline (RC2)

**Product:** Coreside  
**Access date:** 2026-08-03  
**Phase:** Public-beta release candidate 2  
**Public beta:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `3054afb40a4202cda62cfb13f310d8a4ed4fbebf` |
| Dirty | No (clean at baseline generation) |
| Source fingerprint | See `reports/current-source-fingerprint.json` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Observed names | `Coreside Chat AI.zip` / Chat AI(6) |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| SHA-256 | `2856063093f6570bf20f87b082ef9d771880d86d58292a1e3c1f38447c09b07c` |
| Supersedes | `b2f8bf4e…` (Chat AI(5)) |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `f32e3db1ed083c4c69903b536cb15edbb1ba616e8f8affd75c113cd7004e3be6` |
| `src-tauri/Cargo.lock` | `4b118413c6104abc1eab2df0455e2d2218df058c59c075866191c677709d0729` |

## Tool versions

| Tool | Version |
| --- | --- |
| Node | v24.17.0 |
| npm | 11.18.0 |
| rustc | 1.96.1 |
| cargo | 1.96.1 |
| Tauri CLI | 2.11.4 |

## Counts

| Metric | Active |
| --- | --- |
| `src` files | 149 |
| Rust `.rs` | 172 |
| Migrations | 15 |
| E2E specs | 10 |
| Tauri commands | 209 |
| Capabilities | `default.json`, `tool-window.json` |

## RC2 packaged-runtime status (in progress)

| Area | Status |
| --- | --- |
| Protected prompts embedded (`include_str!`) | Implemented this pass |
| Production dotenv restricted | Implemented this pass |
| Crawl4AI AppPaths data root | Implemented this pass |
| Recovery-safe list/preview/restore to `AppPaths.database` | Implemented this pass |
| Scheduler gated off recovery shell | Implemented this pass |
| Desktop/packaged wallpaper pixel proof | Open |
| Full backup media/attachments | Open |
| Tauri AppManifest command ACL | Open |
| Full E2E / packaged smoke / assurance | Open |

## Reports

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
