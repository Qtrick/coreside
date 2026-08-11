# Current Source Baseline (P0.6)

**Product:** Coreside  
**Access date:** 2026-08-11
**Phase:** P0.6 source intake / development audit<br>
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `dd8c60bbbccd1a3b21a3648f16586f7e2adc8a87` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `18165bddbc7b17a128ed3bc1526846640b2f9cafeb9bfb41cd9a28b0aff60d2d` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside main.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside main.zip` |
| Expected SHA-256 | `22a0ab2903fc26c6b17c903ae9f8d7181ed72803bec8f00c447d687462119764` |
| Observed SHA-256 | `22a0ab2903fc26c6b17c903ae9f8d7181ed72803bec8f00c447d687462119764` |
| Status | **present_hash_match** |
| Supersedes | `220c246eeed62133bb6b08d77278248192f4f7503f31004e5ce02861f2ae6412` (`P0.5 supplied Coreside archive`) |
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
| `src` files | 193 |
| Rust `.rs` | 194 |
| Migrations | 24 |
| E2E specs | 20 |
| Tauri commands | 222 |
| Fingerprint files | 716 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | compared |
| Only in active | 4 |
| Only in archive | 0 |
| Changed | 18 |

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
