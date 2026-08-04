# Structured Forms and Context

**Product:** Coreside  
**Phase:** RC3.3 Phase 8  
**Last updated:** 2026-08-03  
**Status:** Typed foundation **Unit Verified** — delimiter is not trust authority

## Trust model

Structured form submissions continue the agent turn as **typed** `StructuredUserInput` parts sealed in Rust:

| Field | Authority |
| --- | --- |
| `trust_class` = `LocalUserGesture` | Set only by `seal_local_user_submission` / `seal_from_ledger_payload` |
| `instruction_eligibility` = `LocalUserContent` | Same seal path — never system/developer elevation |
| `content_hash` | SHA-256 of field JSON, computed in Rust |
| `submission_id` | Generated in Rust |

**Non-authority:** The legacy text delimiter `[STRUCTURED_USER_INPUT trust=…]` must **not** grant structured trust. `structured_trust_from_text` always returns `None`. Spoofed markers in plain user chat are ordinary text.

## Submission path

1. Trusted UI (`submitToAgent` on tool canvas / inline surface) sends a human-readable summary plus a `structuredUserInput` payload on `send_message`.
2. Rust seals the payload (`AgentContentPart::StructuredUserInput`) and stores it on message metadata.
3. BYOK provider adapters flatten via `AgentMessage::provider_text()` / `provider_text_summary` — transport text for models that lack native structured parts.
4. Context ledger `*_form_submit` entries are sealed into typed parts on inject (bounded), not wrapped as trust-bearing delimiters.

Attribution: behavioral reimplementation of Partial Update form → model continuation (MIT, Copyright (c) 2026 Phil Holden). See [PARTIAL_UPDATE_PORT_PROVENANCE.md](./PARTIAL_UPDATE_PORT_PROVENANCE.md). HTML/JS/CDN/iframe form routes remain **rejected**.

## Evidence

- Unit: `cargo test --manifest-path src-tauri/Cargo.toml --lib structured_user_input`
- Report: `reports/structured-form-results.json` → **Unit Verified**

## Remaining gaps

- Native multimodal provider parts (beyond text summary flatten) not required for this slice.
- Desktop E2E for form → turn continuation **not_run**.
- Ledger append is still a Tauri command callable from the UI layer; seal remains the trust gate for model eligibility.
