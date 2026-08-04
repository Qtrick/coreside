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
| Commit | `c520fcfba6acdeed7a14ee1e24dd9ea31f67b4c7` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `92eb087ca187930e45dfd9091dd1fd9daac6a8eb9fe627804f47042983ba6f10` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `75946b05d7778368700c8d827007d83f1cce0006fcedad8cf69b35f37bb23d69` |
| Observed SHA-256 | `75946b05d7778368700c8d827007d83f1cce0006fcedad8cf69b35f37bb23d69` |
| Status | **present_hash_match** |
| Supersedes | `ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228` (prior expected / unavailable baseline) |

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
| `src` files | 153 |
| Rust `.rs` | 175 |
| Migrations | 16 |
| E2E specs | 12 |
| Tauri commands | 212 |
| Fingerprint files | 609 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 5 |
| Only in archive | 0 |
| Changed | 24 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
