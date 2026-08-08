# Current Source Baseline (RC3.11)

**Product:** Coreside  
**Access date:** 2026-08-07  
**Phase:** Public-beta release candidate 3.10 — durable turns, hosted billing, window-scoped tools  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `57f65ae3475b1fd2a1976d0ef329a532e20078ba` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `efa94051db2148fc58d887ad7558bf270c5912090da70550f6e5fb0f2bf266ab` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `8e984965f35cb28807fbb3ad24d1e48b9e6667801df716ab9c294e8e4a33b101` |
| Observed SHA-256 | `10cc325466f85ca09eabcb814622e17a315684fd705330a782f9adc57578f149` |
| Status | **present_hash_mismatch** |
| Supersedes | `3ae9f51473f718532c177e67b55ef1f2fb74cf6ca78e52e15f0e0660e16efadf` (`Coreside Chat AI.zip / prior RC3.10 archive (3ae9f514…)`) |
| Partial Update archive | `Partial Update Main.zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `cdacd042fad79a9b863244cd07f2c238b49cefbf299a8e2d4f7a0f5e5d923dbc` |
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
| `src` files | 192 |
| Rust `.rs` | 193 |
| Migrations | 24 |
| E2E specs | 18 |
| Tauri commands | 222 |
| Fingerprint files | 705 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | archive_unavailable |
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
