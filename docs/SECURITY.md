# Security

## API-key handling

- Consumer keys are entered in the provider setup dialog and stored in OS secure credential storage (`keyring`), never SQLite.
- `.env` remains a development fallback only.
- Frontend receives `keyDetected: boolean` and status strings — never the key.
- Keys are not stored in chat messages, tool definitions, action logs, or exports.
- Logs and error sanitization redact secret-like substrings.

## Action Log

- Base Setting `actionLogMode` is `off` | `always` | `intelligent` (default off) and is user-only (not agent-editable). Legacy `actionLogEnabled` remains synced.
- When enabled, only sanitized operational labels are shown/persisted — never chain-of-thought, prompts, or secrets. Intelligent mode surfaces the log only when substantive work occurred (search, tool changes, etc.).

## Automations & exports

- Automations cannot run shell/JS or target protected resources; they run only while Coreside is open.
- Exports strip credentials and require a user-chosen save path.

## Generated application runtime

- Privileged generated side effects pass through one Rust gateway (`execute_registered_action`).
- Generated UI cannot call arbitrary Tauri commands or forge actor/venue/presence/session.
- Runtime action risk (`read` / `write` / `destructive`) is separate from kernel operation risk.
- Approvals are user-only, TTL 15 minutes, CAS one-time consume; frozen input re-executes on decide.
- Destructive/critical actions cannot receive standing grants; away writes need app-bound standing grants.
- Import packages carry structure, not authority (`docs/APPLICATION_IMPORT_AUTHORITY.md`).
- Audit ledger is always persisted for safety; Action Log Off does not erase integrity events.
- See `docs/GENERATED_APPLICATION_RUNTIME.md` and `docs/REGISTERED_ACTIONS.md`.

## Why AI calls occur in Rust

The webview is an untrusted UI surface. Keeping provider credentials and HTTP in Rust prevents exposure via DevTools, frontend bundles, or XSS-style generative UI attacks.

## Continuity and patch security

- Drafts are isolated by surface/component/window; agents cannot read another surface’s draft.
- Preservation policies are trusted enums; incompatible state is not preserved across types/permissions.
- Patch queue limits and priority ceilings are protected (`core.patch_*`); agents cannot raise them or assign `critical_recovery`.
- Dependency cycles and superseded previews are rejected; stale proposals cannot apply after manual edits without rebase.
- Context ledger entries are project-scoped; model-only visibility is not user-visible chat clutter and must not cross projects.
- Provider conformance profiles are trusted; agents cannot mark a provider conformant or force a weak fallback when a stronger validated mode exists.
- Restart hydration never auto-resumes audio, microphone, or provider generation.

## Component validation

Only registry-listed component types render. Unknown types fail closed. Rust validates structured tool payloads before persistence; the frontend validates again for defense in depth. Each surface also has a normalized capability-pack set: `coreside.core` is implicit, and non-core component types must belong to a pack already assigned to that surface. Unknown or disabled pack IDs fail closed. Existing rows with no assignment are backfilled to the minimum packs their already-trusted definition needs; ordinary patches cannot expand that set. The same rule is checked before a speculative preview paints and before a durable commit.

## Protected core

Reserved identifiers (`core.branding*`, `core.settings*`, `core.navigation`, `core.security`, `core.database`, `core.versioning`, `core.search*`, `core.media*`, `core.wallpaper*`, `core.agent.tool_loop`, `core.projects*`, `core.application_kernel`, `core.recovery_mode`, `core.package_validator`, `core.policy_engine`, and related Application Kernel IDs) cannot be created, updated, deleted, or shadowed by generated tools or Added Settings. See `src-tauri/src/security/protected_resources.rs`.

Base Settings structure (Appearance, AI Agent, Data, Accessibility, About) is product-owned. Theme preference, accent colors, solid backgrounds, borders, text colors, and allowlisted live wallpapers are user preferences and may be changed via the allowlisted `settings_change` agent path. Added Settings are user/tool-owned and cannot use protected IDs.

Brand logo and dock-icon files are not writable through the agent protocol.

## SSRF and outbound fetch

Web Research, page fetch, and media import run in Rust with URL validation before any crawl or HTTP request:

- Rejects `file://`, localhost, loopback, link-local, and RFC1918 private ranges
- Limits redirects, response size, and HTML script stripping for page text extraction
- Local Crawl4AI sidecar (stdio) re-checks URLs; robots.txt always enforced; stealth/proxies disabled
- AI provider keys live in keyring or `.env` only — never SQLite, chat logs, or the crawler process
- Exa search keys use the same keyring → `.env` precedence; never SQLite; never passed to Crawl4AI
- Local Exa monthly budget and search profiles are protected settings (agent cannot change via generic `set_setting`)
- Web Research indexed discovery uses Exa Search (not Exa Agent); see [EXA_INTEGRATION.md](./EXA_INTEGRATION.md) and [CRAWLER_SECURITY.md](./CRAWLER_SECURITY.md)

## Readability and wallpapers

- Wallpapers live under Added Settings → Templates (not Base Settings structure)
- Wallpaper definitions cannot inject arbitrary CSS/JS or override protected semantic tokens
- Readability contrast helpers clamp unsafe color combinations; see [READABILITY.md](./READABILITY.md)

## Media validation

Imported assets must pass magic-byte detection and MIME allowlists. Executables, HTML, and unknown formats are rejected. Content is stored under app data and deduplicated by hash.

## Project isolation

Full-text project context search is scoped by `project_id`. FTS rows outside the project's conversation membership are excluded (fail closed).

## Local data

Conversation and tool data stay on the device in SQLite under the application data directory. Destructive clears require confirmation in Settings.

## Native-window permissions

The main window matches `src-tauri/capabilities/default.json` (`coreside-main-default`, window `main`). Secondary tool windows match `src-tauri/capabilities/tool-window.json` (`coreside-tool-scoped`, windows `tool-*`). Tool windows receive only their bounded command categories; they do not inherit the main-window capability, arbitrary filesystem/network access, or API keys. Tauri merges matching capabilities, so window globs and capability overlap are audited as a security boundary.

## Threats inherited from generative UI

Inspired by Partial Update’s own warnings:

- Prompt injection attempting to exfiltrate secrets → secrets never enter the model-visible webview state
- Auto-submitting loops → action engine loop limits; no model-authored scripts
- Malformed tool JSON → reject change; preserve prior version

## Future sandboxing plan

Later phases may explore sandboxed custom TypeScript or Wasm components. That is explicitly outside this MVP. Until then, declarative registry components remain the only generation path.

## Continuity and patch security

- Drafts are isolated by surface/component/window; agents cannot read another surface’s draft.
- Preservation policies are trusted enums; incompatible state is not preserved across types/permissions.
- Patch queue limits and priority ceilings are protected; agents cannot raise them or assign `critical_recovery` / `direct_user_interaction` / `active_turn_preview`.
- Dependency cycles and superseded previews are rejected; stale proposals cannot apply after manual edits without rebase.
- Context ledger entries are project-scoped; model-only visibility is not user-visible chat clutter and must not cross projects.
- Provider conformance profiles are trusted; agents cannot mark a provider conformant or force a weak fallback when a stronger validated mode exists.
- Restart hydration never auto-resumes audio, microphone, or provider generation.
