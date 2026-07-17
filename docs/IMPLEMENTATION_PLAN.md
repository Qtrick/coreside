# Implementation Plan

## Status

Coreside consumer MVP foundation is implemented as a Tauri 2 + React + TypeScript + Rust desktop application with Gemini as the first AI provider and a declarative personal-tool system.

## Phase checklist

### Phase 1 — Repository and reference inspection

- [x] Inspect workspace (empty; build from scratch)
- [x] Locate and extract Partial Update ZIP to `.reference/` (gitignored)
- [x] Document adopted / adapted / rejected ideas
- [x] Create this plan and acceptance checklist
- [x] code-reviewer-editor review of plan + early code

### Phase 2 — Tauri foundation

- [x] Package metadata and window title: Coreside
- [x] `npm run dev` → Tauri desktop
- [x] Rust / frontend module boundaries
- [x] Minimal capabilities
- [x] Launch verification on this machine

### Phase 3 — Environment and Gemini

- [x] `.env` + `.env.example` + gitignore
- [x] Provider-neutral config with alias precedence
- [x] Gemini adapter + health check + timeout/cancel
- [x] Mock provider for tests
- [x] Secrets never reach frontend
- [x] Missing-key launch verified (`has_key=false`, setup banner)

### Phase 4 — SQLite

- [x] Automatic migrations
- [x] Settings, conversations, messages, tools, versions, state
- [x] Transactions for apply/undo
- [x] DB unit tests

### Phase 5 — Chat

- [x] Sidebar, conversations, composer, markdown, cancel/retry
- [x] Persist messages
- [ ] Live Gemini chat (requires user API key — not present in this environment)

### Phase 6 — Agent protocol

- [x] Prompt files + prompt builder
- [x] Structured response schema + parser
- [x] Fixtures / tests
- [x] Docs (`AGENT_PROTOCOL.md`)

### Phase 7 — Tool renderer

- [x] Component registry (required types including quiz)
- [x] Actions + state persistence
- [x] Error fallbacks

### Phase 8 — Preview / versions / undo

- [x] Preview Apply / Discard
- [x] Version creation + undo
- [x] Mock fixture coverage for create / update / quiz

### Phase 9 — Themes and Settings

- [x] System / Light / Dark + green/amber tokens
- [x] AI connection status + Test connection
- [x] About + destructive data actions

### Phase 10 — Native secondary window

- [x] Open tool in new window (`/#/tool/:id`)
- [x] Window manager code reviewed (unique window policy)

### Phase 11 — Integration testing

- [x] Automated verify suite
- [x] Desktop launch with missing key
- [ ] Live Gemini acceptance flows (blocked without API key)

### Phase 12 — Final review loop

- [x] code-reviewer-editor pass + fixes
- [x] Re-verify

## Acceptance criteria checklist

| ID | Criterion | Status |
| --- | --- | --- |
| A | Fresh local startup (`npm install` / `npm run dev`) | verified |
| B | Missing API key polished behavior | verified |
| C | Real conversation + persistence | blocked — no API key in environment; mock path covered |
| D | Create water tracker tool | covered via mock fixtures + renderer |
| E | Modify existing tool without duplicate | covered via mock update fixture + versioning |
| F | Undo last tool change | covered via DB tests + UI wiring |
| G | Quiz tool | covered via mock 3-question geography fixture |
| H | Themes persist, no blue-purple | implemented (green/amber tokens) |
| I | Native secondary tool window | implemented |
| J | Malformed fixture safety | covered via parser tests |

## Default model

`gemini-3.5-flash` (GA Flash-class; override via `AI_MODEL` / `GEMINI_MODEL`).
