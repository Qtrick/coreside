# Current Source Baseline (P0.3)

**Product:** Coreside  
**Access date:** 2026-08-09  
**Phase:** Public-beta release candidate 3.10 — durable turns, hosted billing, window-scoped tools  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `590705317ad0cf9b827f847ef1e53506ded931eb` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `06cc24b02d800106037fcd40c96f69ec7151f98fbd98e2996745875233f93640` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI (1).zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI (1).zip` |
| Expected SHA-256 | `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` |
| Observed SHA-256 | `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` |
| Status | **present_hash_match_extract_stale** |
| Supersedes | `8e984965f35cb28807fbb3ad24d1e48b9e6667801df716ab9c294e8e4a33b101` (`Coreside Chat AI.zip / prior RC3.11 archive (8e984965…)`) |
| Partial Update archive | `Partial Update Main.zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `f99da7c99dc5e2db93567632e2c36d36796c4b3ea8cf6a575a3f337bb6b9d641` |
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
| Fingerprint files | 711 |

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
