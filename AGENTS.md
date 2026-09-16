# Coreside Codex Guide

## 1. Mandatory Pre-Flight Instruction Chain
Before substantive work, every Codex session MUST read the canonical Cursor rule files completely:
1. `.cursor/rules/project-context.mdc`
2. `.cursor/rules/Coreside-development.mdc`
3. `.cursor/rules/Ponytail-Code-Reduction.mdc`
4. Relevant architecture and security documents in `docs/` (e.g., `docs/BRANDING.md`, `docs/MIGRATION_ASSURANCE.md`).

Treat those files as the authoritative shared Coreside product and development context unless current source code or an explicit newer user instruction supersedes a factual implementation claim. Always inspect actual code before assuming documented functionality exists.

---

## 2. Product Identity & Protected Core
- **Product Name**: Exactly `Coreside`.
- **Nature**: Consumer-first, AI-native desktop personal software environment (Tauri + Rust + React/TypeScript + SQLite + OS Keychain). Conversations evolve into persistent internal tools, surfaces, automations, wallpapers, and workflows.
- **What Coreside Is NOT**: Not a generic chatbot, not an external website builder, not an unrestricted model sandbox, not an IDE, not a provider wrapper, and not a Partial Update clone.
- **Protected Core (Rust is Authority)**:
  - Product branding, application icons, and Dock tile mutations.
  - Base Settings KV, security controls, credential boundaries.
  - Database schema, migrations, outbox, and turn journal.
  - Capability packs and runtime action engine.
  - Generated surfaces NEVER receive unrestricted JavaScript, global CSS, `<script>` tags, CDN imports, iframe permissions, raw Tauri/shell commands, raw filesystem/network access, or secrets.

---

## 3. Provider & Credential Policy
- **Hosted Services**: Coreside-hosted Gemini, OpenRouter, Exa, and future platform credentials belong exclusively in Supabase Edge / server secrets. Never ship or cache hosted keys in the desktop binary or SQLite.
- **User BYOK Keys**: Stored exclusively in the operating-system keychain via `keyring`. Authless Local AI (Ollama) never accesses keychain credentials. Never export or serialize secrets into logs, messages, or diagnostics.

---

## 4. Partial Update Policy
- Study `Partial Update` (`FFATU` / archive inputs) for transferable concepts: complete-unit progressivity, serialized mutations (`withQueueMutation`), generation retry guards, and idempotent restoration.
- Strictly reject its unsafe patterns: arbitrary HTML/CSS/JS execution, remote CDN assets, iframe forms, client secrets in URLs/sessionStorage, and Durable Object single-owner architectures.

---

## 5. Specialist-Agent Workflow
For substantive work, Cursor and Codex sessions must use specialist agents:
1. **test-runner**:
   - Establish baseline: verify archive hashes, Git status, dirty state, migrations, and tooling.
   - Run focused and full checks.
   - Distinguish product defects from environmental limitations.
   - Never edit production source.
   - Report exact commands, exit codes, and counts.
2. **code-reviewer-editor**:
   - Runs on the integrated live diff before final verification.
   - Audits correctness, security boundaries, race conditions, AppKit main-thread dispatch, persistence, rollback, packaging integrity, and accessibility.
   - Patches conclusive defects directly; avoids style-only churn.
   - Re-runs focused checks for all applied patches.

Project-scoped Codex agent definitions reside under `.codex/agents/`.

---

## 6. Development & Evidence Rules
- **Smallest Root-Cause Fix**: Trace all callers; fix shared causes, not just visible symptoms. Preserve working systems and unrelated dirty work.
- **Evidence Integrity**: Never claim `Desktop Verified`, `Packaged Verified`, `Public Beta`, `Production Ready`, payment readiness, security completion, or human approval without matching evidence on the active source fingerprint.
- **Evidence Classes**:
  - *Proven by source inspection*
  - *Proven by automated test*
  - *Proven by development desktop interaction*
  - *Proven by packaged inspection*
  - *Proven by packaged desktop interaction*
  - *Human visual verified*
  - *Unverified* / *Absent* / *Blocked*
- **Readiness Standard**: Product readiness remains `Development Build / Not ready` until every release gate is satisfied.

---

## 7. Mandatory Session Close: Suggested Commit Message
ALWAYS, after every work session, the agent (Codex, Cursor, Antigravity, or any other AI assistant) MUST provide a suggested commit message following Conventional Commits standards:
- **Prefixes**: Standard types such as `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `perf:`, `build:` (optionally with scope e.g., `feat(dock):`, `fix(migration):`).
- **Subject**: Imperative mood, lowercase, concise (50-72 chars max), no trailing period.
- **Body**: Detailed breakdown explaining *what* changed and *why*, referencing specific files, components, models, and evidence/testing outcomes.
- **Footer**: Any breaking change notes, issue references, or release milestone tags.
