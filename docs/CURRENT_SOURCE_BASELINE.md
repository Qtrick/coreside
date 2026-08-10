# Current Source Baseline (P0.4)

**Product:** Coreside  
**Access date:** 2026-08-10  
**Phase:** Public-beta release candidate 3.10 — durable turns, hosted billing, window-scoped tools  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Coreside` |
| Branch | `main` |
| Commit | `9ed96b923f158a25f92104b3c600ca551f8a141a` |
| Dirty | Yes (development evidence only) |
| Source fingerprint | `796c2fd01729e7a18b890f0170de2bb43fa437d0f3ca2b3955f0e635e5a4c5d3` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | `Coreside Chat AI.zip` |
| Path | `/Users/qunyingfan/Downloads/Coreside Chat AI.zip` |
| Expected SHA-256 | `a9327c01cba67655b5af04ecd0d837b980a5af65348580167f613e6099b8194c` |
| Observed SHA-256 | `a9327c01cba67655b5af04ecd0d837b980a5af65348580167f613e6099b8194c` |
| Status | **present_hash_match_extract_stale** |
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
