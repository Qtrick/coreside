# Current Source Baseline (RC3.12)

**Product:** Coreside  
**Access date:** 2026-10-13
**Phase:** RC3.12 source intake / development audit<br>
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `99010ac559085c96048c8ef59b0eed6baf308096` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `024e82342f4872d3d14e2f553fe3813a1b2e6ea77341dbdf78b1004339ad71bf` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `8d455783ca41492259b15f0ed2973c7db14513a8afe352005d5eff8a1eef3155` |
| Observed SHA-256 | `4af1c1d2b6000eb3d8798fbf9da5300f4d3a7c74fd18a5db49313bb379369c28` |
| Status | **present_hash_mismatch** |
| Supersedes | `f2547160c00e36d1274ee9fa2867d33eb05deb58a1ab05775c50ef6b990b1114` (`P0.8 newest supplied Coreside archive`) |
| Partial Update archive | `Partial Update Main (1).zip` / `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| `package-lock.json` | `33d18f751b95af3fa3e259e91f7205c04fab4ad8badaa4db2231535fd80d9abf` |
| `src-tauri/Cargo.lock` | `b64ad87161713f19b77675d96b38081e050c78ec6b8e3d6b3e00f26b49bfbd67` |

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
| `src` files | 230 |
| Rust `.rs` | 216 |
| Migrations | 37 |
| E2E specs | 32 |
| Tauri commands | 248 |
| Fingerprint files | 820 |

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
