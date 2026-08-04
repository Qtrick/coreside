# Local AI (RC3.5)

**Product:** Coreside  
**Status:** Integrated – Not Verified for desktop journeys

## Authless operation

Local presets (`ollama`, `lmstudio`, `vllm`, `llama_cpp`) use `AuthMode::LocalAuthless`.

- Creating a connection does **not** require a fake API key (`ollama`, `none`, `local`, …).
- Secrets are not written to the keychain for authless connections.
- Credential resolution returns `api_key: None` and still marks `source: connection`.

## Endpoint policy

| Mode | Scheme | Host |
| --- | --- | --- |
| Loopback Local AI | HTTP or HTTPS | `127.0.0.1`, `localhost`, `::1` only |
| Private LAN Local AI | HTTP or HTTPS | RFC1918 / ULA — requires explicit LAN class |
| Remote BYOK | HTTPS only | Public hosts; no userinfo; no metadata |

## Current send path notes

- **Ollama:** Descriptor protocol is `ollama_native_chat`. Temporary chat path uses OpenAI-compatible `…/v1/chat/completions` until the native NDJSON adapter is complete.
- **LM Studio / vLLM / llama.cpp:** OpenAI Chat Completions against configured loopback base URL.

## Consumer UI

AI connections picker lists Local AI entries with “no API key”, prefilled loopback base URL, and server health check on save.
