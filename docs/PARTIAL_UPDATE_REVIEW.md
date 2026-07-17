# Partial Update Review

This document records how Coreside studied and selectively adapted ideas from **Partial Update** (MIT License, Copyright (c) 2026 Phil Holden), supplied as the local archive `Partial Update Main.zip`.

Reference extract (not committed): `.reference/partial-update/partialupdate-main/`

## 1. Ideas adopted

| Concept | How Coreside uses it |
| --- | --- |
| AI can produce interactive UI changes, not only Markdown | Agent may return a structured `tool_change` that Coreside renders with trusted components |
| Forms / controls can send structured events back to the agent | Declarative `submitToAgent` actions emit `tool_interaction` events with field values |
| Stable instance identifiers for generated surfaces | Every tool and component has a stable `id` that survives edits |
| Interfaces can be modified across turns | Active tool context is sent with later messages; updates replace the same tool id |
| Clear response protocol | Versioned JSON schema (`schemaVersion: "1"`) instead of free-form scraping |
| Provider logic separated from UI | Rust `AiProvider` trait; React never talks to Gemini directly |
| Targeted updates instead of blind full rebuilds | Tool changes target a known tool id; version history + undo |
| Inspectable / debuggable agent turns | Dev diagnostics: provider, model, prompt version, parse warnings |
| Protect users from runaway request loops | Action-engine loop limits; no model-generated scripts that can auto-submit |

## 2. Ideas adapted

| Partial Update idea | Coreside adaptation |
| --- | --- |
| Delimiter-based multi-message protocol with HTML bodies | Replaced with a single validated JSON agent response |
| Path-like template markers (`/chat/append-message`, `/app/...`) | Replaced with declarative component trees and a component registry |
| Hidden iframe form submission | Replaced with in-process action dispatch + optional `submitToAgent` |
| Debug view of LLM message history | Lightweight diagnostics on agent turns (no Cloudflare debug page) |
| Model selection / env-driven provider choice | Provider-neutral `AI_*` env vars with Gemini as first adapter |

## 3. Ideas rejected

- Cloudflare Workers as the required local runtime
- Durable Objects and hibernatable WebSockets
- Cloudflare D1
- Better Auth / multiuser chat / OAuth identity
- Cloudflare AI Gateway as a requirement
- Direct insertion of unrestricted model-generated HTML, CSS, or JavaScript
- Arbitrary third-party CDN loading (Tailwind, d3, CodeMirror, etc. injected by the model)
- Full-page rewriting by the model
- Cloudflare-specific deployment requirements

## 4. Why unrestricted HTML / CSS / JS / CDN injection is not copied

Partial Update demonstrates a powerful generative-UI idea: the model returns HTML that the page applies via declarative partial updates. That approach also inherits serious consumer risks:

1. **Script execution** — model output (or a prompt-injection attacker) can run arbitrary JavaScript in the host page.
2. **Cost amplification** — clientside loops can spam inference or network requests (called out in Partial Update’s own README).
3. **Supply-chain / CDN trust** — loading arbitrary remote libraries expands the attack surface.
4. **Core integrity** — unrestricted DOM rewrites can break the host application chrome and persistence guarantees.
5. **Desktop trust boundary** — Coreside is a local desktop app with a Rust core holding API keys; untrusted script in the webview is unacceptable for the MVP security model.

## 5. How Coreside’s structured component system is safer

Coreside keeps a **trusted component registry** in TypeScript. The agent may only select and configure known types (`counter`, `quiz`, `checklist`, …). Rust validates structured responses before persistence. The action engine only allows a closed set of local mutations. No model-authored JavaScript is executed. Unsupported types fail closed with a fallback, without corrupting the last good tool version.

## 6. Code copied or substantially adapted

**No Partial Update source files were copied into the Coreside application tree.**

What was used:

- Conceptual study of `README.md`, `spec.md`, `src/initialPrompt.md`, `src/getPrompt.ts`, `src/env.ts`, `src/index.ts` (protocol and architecture), `package.json`, `wrangler.jsonc`, and auth-related modules (to understand what to avoid).
- The MIT license text for attribution (see below).

No HTML templates, Durable Object logic, auth routes, or delimiter parsers were ported.

## 7. Licensing and attribution

Partial Update is MIT-licensed:

> Copyright (c) 2026 Phil Holden

Because Coreside studied the project conceptually and did not copy substantial source code, a dedicated `THIRD_PARTY_NOTICES.md` is not required for Partial Update source. Attribution of inspiration is documented here and in the root `README.md`.

If future work copies Partial Update code verbatim, that notice file must be added and copyright headers preserved.
