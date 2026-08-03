# Protected Prompt Packaging

**Product:** Coreside  
**Access date:** 2026-08-03  
**Status:** Implemented (compile-time embedding)

## Decision

Protected agent prompts are embedded with `include_str!` in `src-tauri/src/ai/prompt_builder.rs`.

## Why

Runtime `CARGO_MANIFEST_DIR/prompts` loading returned empty strings when files were missing, silently degrading packaged agent policy.

## Behavior

- Build fails closed if embedded content is empty (panic on first `load_prompts()`).
- No working-directory or user-profile prompt override.
- Prompt hashes available via `prompt_integrity_report()`.
- Inventory: `reports/protected-prompt-inventory.json`

## Files

- `system.md`
- `tool_builder.md`
- `tool_editor.md`
- `response_rules.md`
- `protected_resources.md`
