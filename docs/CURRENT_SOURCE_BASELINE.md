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
| Commit | `1f5fc88a1d569d32c14e329697964ff01acdd034` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `be0da4e5c7d802cb4dedaf4d1789d4dc114f2022aa5e76fa8ba25e4c7734dc99` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `907a21f13ccbea5d7cbf7793bb5cb53098fa72836756b8f4c9a15569ef7c88c4` |
| Observed SHA-256 | `907a21f13ccbea5d7cbf7793bb5cb53098fa72836756b8f4c9a15569ef7c88c4` |
| Status | **present_hash_match** |
| Supersedes | `dd1370465031e1cf4e4e7317eed37f03301ec49ece5ca6135a439042dd1dcccb` (prior expected / unavailable baseline) |

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
| Tauri commands | 211 |
| Fingerprint files | 601 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 0 |
| Only in archive | 0 |
| Changed | 17 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
