# Environment Configuration Policy

**Product:** Coreside  
**Access date:** 2026-08-03

## Production

Normal release builds **do not** load `.env` from:

- Current working directory
- Parent directories
- Source repository root

Configuration sources:

- Built-in non-secret defaults
- OS-secure provider connections (BYOK)
- Explicit local-model / provider Settings
- Hosted configuration where applicable
- Explicit process environment only when set by the deployer

## Development

`.env` loading is allowed when:

- `cfg!(debug_assertions)` is true, **or**
- `CORESIDE_ALLOW_DOTENV=1`

When allowed, candidates are limited to:

- Project root next to `src-tauri`
- `src-tauri/.env`
- Current directory `.env` (no parent walk)

`dotenvy::dotenv()` cwd fallback is **not** used.

## Tests

- `config::env::tests::dotenv_policy_is_debug_or_explicit`
- Packaged malicious-CWD test remains required for final release evidence
