# Partial Update Review

This document records how Coreside studied and selectively adapted ideas from **Partial Update** (MIT License, Copyright (c) 2026 Phil Holden), supplied as the local archive `Partial Update Main.zip`.

Reference extract (not committed): `.reference/partial-update/partialupdate-main/`

For the exhaustive 2026-07-18 audit, see:

- `docs/PARTIAL_UPDATE_EXHAUSTIVE_AUDIT.md`
- `docs/PARTIAL_UPDATE_FILE_ANALYSIS.md`
- `docs/PARTIAL_UPDATE_VIDEO_ANALYSIS.md`
- `docs/PARTIAL_UPDATE_FEATURE_MATRIX.md`
- `docs/PARTIAL_UPDATE_ADOPTION_DECISIONS.md`
- `docs/GENERATIVE_INTERFACE_RUNTIME_V2.md`
- `docs/GENERATIVE_UI_SECURITY_MODEL.md`
- `reports/partial-update-feature-matrix.json`

## 1. Ideas adopted (Runtime V2)

| Concept | How Coreside uses it |
| --- | --- |
| AI produces interactive UI changes, not only Markdown | Schema v1 `tool_change` + schema v2 `operations[]` |
| Forms / controls send structured events back | `submitToAgent` + rich form pack |
| Stable instance identifiers | `instance_id` on every surface |
| Interfaces modified across turns | Surface revisions + patches |
| Clear response protocol | Versioned JSON (`"1"` and `"2"`) |
| Provider logic separated from UI | Rust `AiProvider` trait |
| Targeted updates instead of blind full rebuilds | `component.update_props` + revision checks |
| Inspectable agent turns | Developer diagnostics (redacted) |
| Protect users from runaway loops | EventBus limits + action depth + queue |

## 2. Ideas adapted

| Partial Update idea | Coreside adaptation |
| --- | --- |
| Delimiter multi-message HTML protocol | Validated JSON operations + NDJSON stream frames |
| Path markers (`/app/...`) | Stable surface/component IDs |
| Hidden iframe form submission | In-process action dispatch |
| CDN libraries | Bundled capability packs |
| Fork pages | Local chat branches + read-only snapshots |
| DO WebSocket multiuser | Adapted as deterministic Audience routing (`CurrentUser`, `CurrentSurface`, `CurrentChat`, `CurrentProject`, and `FutureParticipants` journal record) with project/chat isolation |

## 3. Ideas rejected

- Unrestricted model-generated HTML / CSS / JavaScript
- Arbitrary CDN injection
- Cloudflare Workers / Durable Objects / D1 as required runtime
- Production multiuser networking and cloud auth for consumer parity
- Full protected-chrome redesign by the model

## 4. Code copied

Partial Update source files are **not vendored** into the Coreside tree. Valuable algorithms are **directly translated or substantially adapted** into Coreside’s trusted stack; unsafe HTML/JS/CDN/iframe execution is rejected. See `THIRD_PARTY_NOTICES.md`, `docs/PARTIAL_UPDATE_DIRECT_PORT_POLICY.md`, and `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md`.
