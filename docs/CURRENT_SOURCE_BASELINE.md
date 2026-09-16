# Current Source Baseline (RC3.12)

**Product:** Coreside  
**Access date:** 2026-09-16
**Phase:** RC3.12 source intake / development audit<br>
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `1839c1bd1f8baad098dc3fb2b60de7e52c09a863` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `b1afd5ce6e10ac82bbd86efacf33c8ba88724978f0019886238dc21033ad8c06` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `8d455783ca41492259b15f0ed2973c7db14513a8afe352005d5eff8a1eef3155` |
| Observed SHA-256 | `n/a` |
| Status | **zip_unavailable_extract_present** |
| Supersedes | `f2547160c00e36d1274ee9fa2867d33eb05deb58a1ab05775c50ef6b990b1114` (`P0.8 newest supplied Coreside archive`) |
| Partial Update archive | `Partial Update Main (1).zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

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
| `src` files | 198 |
| Rust `.rs` | 196 |
| Migrations | 24 |
| E2E specs | 20 |
| Tauri commands | 223 |
| Fingerprint files | 723 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared_against_prior_extract |
| Only in active | 29 |
| Only in archive | 1 |
| Changed | 84 |

## Generation command

```bash
npm run audit:current-source -- --check

# Intentionally regenerate source-controlled evidence
npm run audit:current-source:write
```

Reports:

- `reports/current-source-baseline.json`
- `reports/current-source-fingerprint.json`
- `reports/active-versus-uploaded-coreside.json`
- `reports/report-freshness-inventory.json`
