# Current Source Baseline (RC3)

**Product:** Coreside  
**Access date:** 2026-08-03  
**Phase:** Public-beta release candidate 3  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `74793b1d69d8e81e3d6a560018b3d4ce5021e243` |
| Dirty at kickoff snapshot | No (clean at RC3 start; report JSON edits may dirty the tree) |
| Source fingerprint | `d31e19940a397434376dcf00233bcd35d55821dd8c98351bd38c8202025e1111` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Expected path | `/Users/qunyingfan/Downloads/Coreside Chat AI (1).zip` |
| Expected SHA-256 | `b0f80f58d3d45f381a94956a5fec9e5cec4736cd585e12e0460f01734a18ada6` |
| Status | **archive_unavailable** (file not present on disk; byte-level archive vs workspace diff unavailable) |
| Supersedes | `2856063093f6570bf20f87b082ef9d771880d86d58292a1e3c1f38447c09b07c` (RC2 / Chat AI(6)) |

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
| Tauri commands | 210 |
| Capabilities | `default.json`, `tool-window.json` |

## RC2 packaged-runtime (do not regress)

| Area | Status |
| --- | --- |
| Protected prompts embedded (`include_str!`) | Implemented (keep) |
| Production dotenv restricted | Implemented (keep) |
| Crawl4AI AppPaths data root | Implemented (keep) |
| Recovery-safe list/preview/restore to `AppPaths.database` | Implemented (keep) |
| Scheduler gated off recovery shell | Implemented (keep) |
| Unicode `sanitize_error` | Implemented (keep) |

## RC3 open work (not fixed / not beta-verified)

| Area | Status |
| --- | --- |
| Desktop/packaged wallpaper pixel proof | Open |
| Full backup media/attachments | Code landed; restore/E2E not beta-verified |
| Tauri AppManifest command ACL (`build.rs`) | Code landed; raw-invoke denial E2E pending |
| Attachment/media roots via `AppPaths` | Code landed; packaged proof pending |
| Attachment magic-byte hardening | Code landed; packaged proof pending |
| Full E2E / packaged smoke / assurance | Open |
| RC3 uploaded archive verification | Blocked (`archive_unavailable`) |

## Reports

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
