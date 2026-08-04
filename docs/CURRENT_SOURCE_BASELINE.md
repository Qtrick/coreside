# Current Source Baseline (RC3)

**Product:** Coreside  
**Access date:** 2026-08-04  
**Phase:** Public-beta release candidate 3  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `25e6c80e8ec7c0b9f43829f4efa053b56508e257` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `b6afade1134a3a70e1d0d07cd9be007e6105bea52f9592b2bccc4c0663b65ab1` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `9d951b9f8ab53b24fde55bcfa49f8b4005020ccc045d0e8226be5fbbf9d0ab06` |
| Observed SHA-256 | `9d951b9f8ab53b24fde55bcfa49f8b4005020ccc045d0e8226be5fbbf9d0ab06` |
| Status | **present_hash_match_extract_stale** |
| Supersedes | `a7d6c7ab2e5ac1c58b83b1607a49171338dbfba927b29ed549aa5d3510f0317d` (prior expected / unavailable baseline) |

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
| `src` files | 177 |
| Rust `.rs` | 181 |
| Migrations | 18 |
| E2E specs | 16 |
| Tauri commands | 222 |
| Fingerprint files | 663 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | extract_stale_or_unmarked |
| Only in active | 0 |
| Only in archive | 0 |
| Changed | 0 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
