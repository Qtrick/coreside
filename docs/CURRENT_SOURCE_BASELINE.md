# Current Source Baseline (P0.5)

**Product:** Coreside  
**Access date:** 2026-08-10  
**Phase:** P0.5 source intake / development audit<br>
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `1bd2a3e8376afdf847cec55ae90fe9e219eee605` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `c0da070e975788ca2881dd74f503eb98ef9a0d32a6d9246a35dc63e51d4d81dd` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `220c246eeed62133bb6b08d77278248192f4f7503f31004e5ce02861f2ae6412` |
| Observed SHA-256 | `220c246eeed62133bb6b08d77278248192f4f7503f31004e5ce02861f2ae6412` |
| Status | **present_hash_match** |
| Supersedes | `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` (`P0.3 supplied Coreside archive`) |
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
| E2E specs | 20 |
| Tauri commands | 222 |
| Fingerprint files | 714 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 2 |
| Only in archive | 0 |
| Changed | 10 |

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
