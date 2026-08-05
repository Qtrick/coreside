# Current Source Baseline (RC3.6)

**Product:** Coreside  
**Access date:** 2026-08-04  
**Phase:** Public-beta release candidate 3.6 — hosted AI + local privacy routing  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `2b502278ede0d0db57fe16f77ef1e282a92fd0d1` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `f5922bfb542f59b2bab4cbe3e14f217219527b0dc4c5ed5aff45d3e93de3579f` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `724cd17ca7249f5fd94b5711d840b8bb1cc6ffaccec14559e85d1f0e93836c7a` |
| Observed SHA-256 | `724cd17ca7249f5fd94b5711d840b8bb1cc6ffaccec14559e85d1f0e93836c7a` |
| Status | **present_hash_match** |
| Supersedes | `e8325a54af8a98889272a397dd8afa9523c3303531837e4eab3bbf38a930c70c` (`Coreside Chat AI (1).zip / RC3.5 archive`) |
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
| `src` files | 178 |
| Rust `.rs` | 190 |
| Migrations | 19 |
| E2E specs | 18 |
| Tauri commands | 222 |
| Fingerprint files | 676 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 6 |
| Only in archive | 0 |
| Changed | 56 |

## Generation command

```bash
npm run audit:current-source
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
