# Application Audit Ledger

**Status:** Implemented  
**Table:** `runtime_audit_events`

## Purpose

Bounded local safety ledger for generated-application execution. Separate from the optional user-facing Action Log setting.

Critical approval/execution integrity data is always persisted even when Action Log is Off.

## Event fields

`id`, `kind`, `actor`, `venue`, `presence`, `application_id`, `project_id`, `conversation_id`, `run_id`, `action_name`, `input_preview`, `outcome`, `risk`, `decision_source`, `approval_id`, `grant_id`, `detail`, `duration_ms`, `created_at`

## Redaction

Secrets, key-shaped strings, and oversized previews are redacted/bounded before storage. No chain-of-thought, raw provider payloads, or keychain values.

## Retention

- Soft ceiling ~2000 rows with trim on append
- `kernel_clear_audit_events` refuses while live approvals or active grants exist

## UI

Recent events appear in Application Details. Global clear is available from App permissions settings when safe.
