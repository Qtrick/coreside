# P0.4 Closure Evidence — 2026-08-09/10

## Objective

Secure Declarative Update proof: Journey 19 Desktop Verified progressive preview, Chrome/WICG crosswalk, provenance intakes, security review, dev-runner hardening, fresh release package.

## Source identity

| Item | Value |
| --- | --- |
| Branch | `main` |
| Commit | `9ed96b923f158a25f92104b3c600ca551f8a141a` |
| Dirty | yes (P0.4 worktree) |
| Coreside archive (P0.4) | `a9327c01cba67655b5af04ecd0d837b980a5af65348580167f613e6099b8194c` |
| Historical P0.3 | `7e335f187899d40a70dfc5738ed49f28b1177f18177d5e56a4489b33834bbd4d` (preserved in `reports/source-intakes.json`) |
| Partial Update | `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` |
| Source fingerprint | `796c2fd01729e7a18b890f0170de2bb43fa437d0f3ca2b3955f0e635e5a4c5d3` |

## P0.3 adaptive update

User confirmed `npm run dev` adaptive Dock icon — **Human Accepted**. Architecture unchanged this pass.

## Journey 19

- Suite: `progressive-surface-preview`
- Result: **PASS**
- Evidence: `reports/progressive-preview-results.json` (fingerprint match)
- Paint before completion + durable note commit verified
- Root-cause fixes: Rust `value_key` on ToolComponent; TextInput `stateKeyFor`; durable `state.set` mirrors `tool_state`

## Journey 20

- Suite: `progressive-preview-cancel`
- Result: **PASS**
- Evidence: `reports/progressive-preview-cancel-results.json`

## Partial Update matrix movement

- `PU-PROGRESSIVE-PREVIEW` → **Desktop Verified**
- `CS-PACKS` → **Unit Verified** (existing pack tests registered in audit)

## Security

- Main-agent completion after subagent usage limit: `reports/security-findings-p0.4-2026-08-09.json`
- Sensitive kernel commands: main-window label enforcement expanded

## Remaining

- Packaged progressive preview (release harness path)
- Invalid-terminal Desktop journey
- Dev/release data isolation implementation (design documented)
- CSP Google Fonts / style unsafe-inline deferred
- Fresh release evidence / doctor commit stamp
