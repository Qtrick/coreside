# Security

## API-key handling

- Keys load from `.env` in the Rust process only (`AI_API_KEY` / `GEMINI_API_KEY`).
- Frontend receives `keyDetected: boolean` and status strings — never the key.
- Keys are not stored in SQLite, chat messages, tool definitions, or settings IPC.
- Logs and error sanitization redact secret-like substrings.

## Why AI calls occur in Rust

The webview is an untrusted UI surface. Keeping provider credentials and HTTP in Rust prevents exposure via DevTools, frontend bundles, or XSS-style generative UI attacks.

## No arbitrary scripts

The agent must not return executable JavaScript for the host app. Coreside does not `eval` model output, inject `<script>` tags, or load arbitrary CDNs requested by the model.

## Component validation

Only registry-listed component types render. Unknown types fail closed. Rust validates structured tool payloads before persistence; the frontend validates again for defense in depth.

## Local data

Conversation and tool data stay on the device in SQLite under the application data directory. Destructive clears require confirmation in Settings.

## Native-window permissions

Secondary tool windows use the same capability set as the main window. They do not receive elevated filesystem or network privileges and do not receive API keys.

## Threats inherited from generative UI

Inspired by Partial Update’s own warnings:

- Prompt injection attempting to exfiltrate secrets → secrets never enter the model-visible webview state
- Auto-submitting loops → action engine loop limits; no model-authored scripts
- Malformed tool JSON → reject change; preserve prior version

## Future sandboxing plan

Later phases may explore sandboxed custom TypeScript or Wasm components. That is explicitly outside this MVP. Until then, declarative registry components remain the only generation path.
