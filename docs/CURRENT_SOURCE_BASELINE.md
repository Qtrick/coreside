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
| Commit | `a491887e2a80d21619301ed6049de7f29ad9b44b` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `4d9e8454a5e9907e216f73252598ab544aa4f802eef1fb4e335b341b24ee4aea` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `a7d6c7ab2e5ac1c58b83b1607a49171338dbfba927b29ed549aa5d3510f0317d` |
| Observed SHA-256 | `a7d6c7ab2e5ac1c58b83b1607a49171338dbfba927b29ed549aa5d3510f0317d` |
| Status | **present_hash_match** |
| Supersedes | `10d7c5110a12525b53a48ee3dc66bf8d3a054c243553bc4ee9a1db27bc8b86b5` (prior expected / unavailable baseline) |

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
| `src` files | 158 |
| Rust `.rs` | 177 |
| Migrations | 16 |
| E2E specs | 13 |
| Tauri commands | 213 |
| Fingerprint files | 619 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 6 |
| Only in archive | 0 |
| Changed | 34 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
