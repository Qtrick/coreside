# Coreside Development Guidelines for Claude Code

## Project Overview
Coreside is a consumer-first, AI-native desktop personal software environment built with Tauri (v2) + Rust + React 18 / TypeScript + SQLite + OS Keychain.
In Coreside, conversational interactions progressively assemble into persistent internal software tools, surfaces, automations, and wallpapers.

## Commands
- Typecheck: `npm run typecheck`
- Lint: `npm run lint`
- Frontend tests: `npm test`
- Rust check: `npm run check:rust`
- Rust tests: `cargo test` (or `npm run test:rust`)
- Web build: `npm run build:web`

## Core Architectural Guardrails
1. **Rust is the Authoritative Kernel**:
   - Manages SQLite schema, migrations, turn journal, outbox, and security controls.
   - Enforces action authorization and credential boundaries (keychain storage via keyring).
   - Generated interfaces NEVER receive unrestricted JavaScript, arbitrary HTML `<script>` tags, CDN imports, iframes, raw shell commands, or arbitrary network access.
2. **Declarative Runtime (v2)**:
   - UI surfaces are strictly declarative component trees.
   - Incremental modifications must use targeted operations (`component.update_props`, `component.insert`, `component.remove`, `component.move`, `state.patch`) to preserve user focus, scroll, and draft state.
   - Full tool replacement should be reserved only for explicit whole-application redesigns.
3. **Search & Research Security**:
   - All external URL fetches must strictly pass SSRF validation (`validate_public_http_url`).
   - Private IP ranges (RFC 1918, Class E, loopback, link-local, carrier-grade NAT, benchmarking subnets, 6to4/Teredo decapsulated IPv4) are strictly blocked.
   - Web content is treated as untrusted data with clear boundary tags and prompt injection guards.

## Shared Engineering Discipline
1. **Inspect before modifying**: Always inspect real source code first; do not assume documented functionality exists.
2. **Implement, do not merely recommend**: Resolve defects directly with minimal, production-quality fixes and regression tests.
3. **Evidence integrity**: Never claim production readiness or visual verification without active, reproducible evidence.
4. **No unprompted git commands**: Never execute `git add`, `git commit`, or `git push` automatically.
5. **Session Close**: Always conclude with `git status --short` and a suggested Conventional Commit.

## Suggested Commit Message Format (STRICT)
- Standard Conventional Commits format (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `perf:`).
- Subject must be imperative, lowercase, concise (under 72 chars), with no trailing period.
- **NEVER use double quotes (`"`) anywhere in the commit message.**
- **NEVER wrap the subject line in quotation marks (single or double).**
- **NEVER wrap the subject line in Markdown code blocks or backticks.**
- **NEVER include `git commit -m` or shell wrappers unless explicitly asked.**
- Present the subject on a single, plain line directly following `Suggested Conventional Commit:`:

Suggested Conventional Commit:
feat(scope): concise imperative subject
