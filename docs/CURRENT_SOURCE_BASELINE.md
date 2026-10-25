# Current Source Baseline (RC3.12)

**Product:** Coreside  
**Access date:** 2026-10-24
**Phase:** RC3.12 source intake / development audit<br>
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `7b4413a8b6ea36bbe906ab3258e0a36cd7dbf9eb` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `2b2cab71445617056becd1e8ed334eaf71c6b173f8845fc03e6feb33c2fe0109` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `8d455783ca41492259b15f0ed2973c7db14513a8afe352005d5eff8a1eef3155` |
| Observed SHA-256 | `n/a` |
| Status | **archive_unavailable** |
| Supersedes | `f2547160c00e36d1274ee9fa2867d33eb05deb58a1ab05775c50ef6b990b1114` (`P0.8 newest supplied Coreside archive`) |
| Partial Update archive | `Partial Update Main (1).zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `f9e699683884804c9672d22f1ae160962e2f5d6077194c46bffdea1eee4824d3` |
| `src-tauri/Cargo.lock` | `cd57a7c97fd3227fcb1939087a1e03a05f8629f0091e1f3e504dcc0dc5f95c70` |

## Tool versions

| Tool | Version |
| --- | --- |
| Node | v24.17.0 |
| npm | 12.2.0 |
| rustc | rustc 1.96.1 (31fca3adb 2026-06-26) |
| cargo | cargo 1.96.1 (356927216 2026-06-26) |
| Tauri CLI | tauri-cli 2.11.4 |

## Counts

| Metric | Active |
| --- | --- |
| `src` files | 258 |
| Rust `.rs` | 226 |
| Migrations | 42 |
| E2E specs | 42 |
| Tauri commands | 263 |
| Fingerprint files | 890 |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | archive_unavailable |
| Only in active | 0 |
| Only in archive | 0 |
| Changed | 0 |

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
