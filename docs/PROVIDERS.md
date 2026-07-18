# Providers

Coreside uses a **bring-your-own-key (BYOK)** flow as the primary consumer setup.

## Supported providers

- Google Gemini
- OpenAI
- Anthropic
- OpenRouter
- Custom OpenAI-compatible endpoints (requires base URL)

## Credential precedence

1. Active securely stored user credential (OS keychain / credential manager)
2. Explicit provider connection selected for a test
3. Provider-neutral / provider-alias `.env` development configuration
4. Not configured

Keys are never stored in SQLite. Only non-secret metadata lives in `provider_connections`.

## Setup

Base Settings → **AI Providers** → **Manage providers**, or send a chat message when no key is configured.

Provider-specific grey placeholders come from `provider_key_hints` (for example Gemini `AIza…`). Validation is by connection test, not prefix matching alone.

## Development `.env`

`.env` remains a developer fallback. Settings may show “Using development environment credential” without displaying the key.
