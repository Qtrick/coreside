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
| Commit | `41fd4595db420eac8b1c5cc30723f1dd377608f4` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `432bd1256464df1daa367b3cd68763973c49d8f21edc2ac3b1a2853a20524536` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside main.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside main.zip` |
| Expected SHA-256 | `ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228` |
| Observed SHA-256 | `ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228` |
| Status | **present_hash_match** |
| Supersedes | `907a21f13ccbea5d7cbf7793bb5cb53098fa72836756b8f4c9a15569ef7c88c4` (prior expected / unavailable baseline) |

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
| Rust `.rs` | 175 |
| Migrations | 16 |
| E2E specs | 11 |
| Tauri commands | 211 |
| Fingerprint files | 603 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 2 |
| Only in archive | 0 |
| Changed | 19 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
