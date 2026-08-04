# Current Source Baseline (RC3.5)

**Product:** Coreside  
**Access date:** 2026-08-04  
**Phase:** Public-beta release candidate 3.5 — frontier provider platform  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `ea9ad672e8bc97eb60dd6d9b62ee0856807fcc8b` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `94ad9242e20ed0235002dcf89983fadc91ded3464e90a94000c2487cc5b7e16d` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI (1).zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI (1).zip` |
| Expected SHA-256 | `e8325a54af8a98889272a397dd8afa9523c3303531837e4eab3bbf38a930c70c` |
| Observed SHA-256 | `e8325a54af8a98889272a397dd8afa9523c3303531837e4eab3bbf38a930c70c` |
| Status | **present_hash_match** |
| Supersedes | `9d951b9f8ab53b24fde55bcfa49f8b4005020ccc045d0e8226be5fbbf9d0ab06` (`Coreside Chat AI(12).zip / prior RC3 zip`) |
| Partial Update archive | `Partial Update Main (1).zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

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
| Rust `.rs` | 185 |
| Migrations | 19 |
| E2E specs | 16 |
| Tauri commands | 222 |
| Fingerprint files | 668 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 5 |
| Only in archive | 0 |
| Changed | 15 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
