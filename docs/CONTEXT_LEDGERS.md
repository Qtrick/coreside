# Context Ledgers

**Product:** Coreside  
**Code:** `runtime_v2/context_ledger.rs`

Separate stores:

| Ledger | Visibility | Purpose |
| --- | --- | --- |
| Chat messages | visible | User-facing conversation |
| Context ledger | `model_context_only` / `status_only` / `developer_only` | Structured agent context |
| Operation history | app_transactions | Applied ops |
| Diagnostics | redacted | Developer Mode |

Entries are project-scoped, bounded (`MAX_CONTEXT_LEDGER_PER_CONVERSATION`), and compacted by summary. Raw keystroke spam is not stored. Silent mode cannot hide errors, permissions, or destructive confirmations.
