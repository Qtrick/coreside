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
| Commit | `c3e2a01a3883fc2ef4e065bf955b750ecf8f7055` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `5022bb80bbc730d652a2f6399832cb7544098a2cf58ad60d15b03a73941d41dc` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `dd1370465031e1cf4e4e7317eed37f03301ec49ece5ca6135a439042dd1dcccb` |
| Observed SHA-256 | `n/a` |
| Status | **zip_unavailable_extract_present** |
| Supersedes | `b0f80f58d3d45f381a94956a5fec9e5cec4736cd585e12e0460f01734a18ada6` (prior expected / unavailable baseline) |

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
| rustc | rustc 1.96.1 (31fca3adb 2026-06-26) |
| cargo | cargo 1.96.1 (356927216 2026-06-26) |
| Tauri CLI | tauri-cli 2.11.4 |

## Counts

| Metric | Active |
| --- | --- |
| `src` files | 149 |
| Rust `.rs` | 173 |
| Migrations | 16 |
| E2E specs | 11 |
| Tauri commands | 210 |
| Fingerprint files | 600 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared_against_prior_extract |
| Only in active | 5 |
| Only in archive | 0 |
| Changed | 27 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
