# Partial Update Direct Port Policy

**Product:** Coreside  
**Access date:** 2026-08-03  
**Partial Update archive SHA-256:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607`  
**License:** MIT — Copyright (c) 2026 Phil Holden

## Intent

Coreside directly ports valuable Partial Update *behavior and algorithms* into a trusted Tauri + Rust + React architecture. Unsafe execution mechanisms are rejected.

## Classification

| Class | When to use |
| --- | --- |
| Direct translated port | Algorithms/state machines that translate safely (queue serialization, incremental parse, stale-active recovery) |
| Substantially adapted port | Same structure with different persistence/security (SQLite vs Durable Object) |
| Behavioral reimplementation | User value without unsafe DOM/HTML/JS/iframe mechanisms |
| Existing Coreside equivalent retained | Already correct trusted path |
| Deliberately rejected | Raw HTML/JS/CSS, CDN, hidden iframe, arbitrary selectors, shell, SQL, model-controlled auth |
| Deferred | Public multiuser collaboration not needed for local-first beta |

## Rejected mechanisms (non-negotiable)

- Raw model-generated HTML / CSS / JavaScript execution
- Script tags, CDN injection, hidden iframe forms
- Arbitrary DOM selectors / `innerHTML`
- Model-controlled native paths, Tauri commands, or permissions
- Shell, general filesystem, arbitrary SQL, unbounded network loops

## Provenance requirements

For every direct or substantial translation:

1. MIT notice retained in `THIRD_PARTY_NOTICES.md`
2. File-level attribution comment naming Partial Update source path/symbol
3. Entry in `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md` and `reports/partial-update-port-map.json`
4. Security/persistence/concurrency deltas recorded honestly

## Current port status (RC3.2)

See `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md` and `reports/partial-update-port-map.json`.
