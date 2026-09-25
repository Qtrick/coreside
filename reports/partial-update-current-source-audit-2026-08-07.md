# Partial Update Current-Source Audit

**Product:** Coreside<br/>
**Access date:** 2026-08-07<br/>
**Generated:** 2026-09-25T16:59:41.976Z<br/>
**Commit:** `e9c961923b52420646657888c44872ff90e6e69a`<br/>
**Dirty:** yes<br/>
**Public beta:** **NOT READY**

## Archive

| Field | Value |
| --- | --- |
| Path | `/Users/qunyingfan/Downloads/Partial Update Main.zip` |
| Expected SHA-256 | `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |
| Observed SHA-256 | `n/a` |
| Status | **archive_unavailable** |
| Source root | `.reference/partial-update/partialupdate-main` (reference_reuse) |

## Status counts

| Status | Count |
| --- | --- |
| Unit Verified | 13 |
| Intentionally Rejected | 5 |
| Deferred | 2 |
| **Total** | **20** |

## Highest-value remaining secure parity gap

**PU-PROGRESSIVE-PREVIEW — Progressive generated-surface preview** (Unit Verified)

Progressive trusted surface paint is Unit Verified with SQLite cancel/incomplete rollback evidence (PreviewTransaction + overlay + ToolCanvas) but lacks desktop/packaged progressive-surface journeys — Journey 12 covers text streaming only.

## Features

| ID | Feature | Status | Value | Note |
| --- | --- | --- | --- | --- |
| `PU-STREAM` | Provider streaming (live deltas) | **Unit Verified** | 10 | Historical desktop evidence in true-streaming-results.json not applied (fingerprint/commit mismatch). |
| `PU-PARSE` | NDJSON / UpdateStreamParser incremental parse | **Unit Verified** | 9 | — |
| `PU-PROGRESSIVE-PREVIEW` | Progressive generated-surface preview | **Unit Verified** | 10 | Historical desktop evidence in progressive-preview-results.json not applied (fingerprint/commit mismatch). |
| `PU-PREVIEW-TXN` | Preview transactions (speculative, non-durable) | **Unit Verified** | 9 | — |
| `PU-QUEUE` | Agent queue + queue UI | **Unit Verified** | 8 | — |
| `PU-REPLAY` | History Replay (paced, read-only) | **Unit Verified** | 7 | ReplayPlayer treated as read-only paced stepper (not PU stream re-execution). |
| `CS-BRANCH` | Conversation branches / forks | **Unit Verified** | 8 | — |
| `PU-FORMS` | Structured forms → model (StructuredUserInput) | **Unit Verified** | 9 | — |
| `CS-CONTEXT-LEDGER` | Context ledger prompt injection | **Unit Verified** | 9 | — |
| `CS-ACTION-LOG` | Action Log / Inspector | **Unit Verified** | 6 | — |
| `PU-HTML-RESP` | Generated HTML bodies in protected webview | **Intentionally Rejected** | 9 | Arbitrary HTML in the protected webview is rejected; Coreside uses trusted declarative components. |
| `PU-JS` | Arbitrary generated JavaScript | **Intentionally Rejected** | 2 | Model-generated script execution is rejected; registered actions only. |
| `PU-CDN` | CDN Tailwind / D3 / CodeMirror injection | **Intentionally Rejected** | 3 | CDN script/link injection rejected; bundled capability packs only. |
| `PU-FORM-ROUTE` | Hidden iframe form POST with clientSecret | **Intentionally Rejected** | 8 | iframe form POST + clientSecret trust path rejected; typed StructuredUserInput is the secure equivalent. |
| `PU-SCOPED-CSS` | Arbitrary generated CSS affecting protected UI | **Intentionally Rejected** | 5 | Global/model CSS on protected chrome rejected; semantic tokens + capability styles only. |
| `PU-MULTIPLAYER` | Multiuser / Durable Object collaboration | **Deferred** | 4 | Public multiuser collaboration deferred; local-first consumer beta priority. |
| `PU-AUTH` | Hosted auth / Better Auth stack | **Deferred** | 3 | Hosted identity deferred relative to local BYOK consumer track. |
| `CS-TXN` | Trusted application transactions | **Unit Verified** | 9 | — |
| `CS-PACKS` | Capability packs (trusted components) | **Unit Verified** | 9 | — |
| `PU-CHANNEL-SCOPE` | Scoped turn / Channel delivery (vs global broadcast) | **Unit Verified** | 8 | — |

## Methodology

- Content probes on Partial Update and Coreside trees (mustMatch/anyMatch).
- File existence alone never upgrades status.
- Unit Verified requires substantive assert/expect tests or unit evidence reports with impl still present.
- Desktop/Packaged Verified require evidence reports matching current sourceFingerprint (or fresh e2e journeys).
- Historical desktop claims with drifted fingerprints are noted but not applied.
- HTML/JS/CSS/CDN/iframe form ports classified Intentionally Rejected when reject evidence holds.

## Commands

```bash
npm run audit:partial-update
# optional:
PARTIAL_UPDATE_ARCHIVE="$HOME/Downloads/Partial Update Main.zip" npm run audit:partial-update
```

Reports:

- `reports/partial-update-feature-matrix.json`
- `reports/partial-update-current-source-audit-2026-08-07.md`
