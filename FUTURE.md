# Coreside Feature Pipeline & Future Architecture (FUTURE.md)

This document is the canonical record of **planned, dormant, foundational, in-progress, blocked, and completed feature pipelines** across Coreside. All AI agents (Codex, Cursor, Antigravity, Claude Code) must consult this document before planning feature work.

---

## 1. Operating Rules & Status Conventions

### Status Vocabulary
- **PLANNED** — Design identified and architecturally anticipated; implementation not yet started.
- **FOUNDATION** — Architectural extension points, data structures, or types exist; user-facing UI is strictly dormant/hidden.
- **IN PROGRESS** — Active implementation underway in the current development campaign.
- **BLOCKED** — Concrete external, environment, or platform blocker prevents completion.
- **COMPLETED** — Implemented, reviewed, and adequately verified with passing automated tests.
- **DEFERRED** — Deliberately postponed outside current consumer desktop milestones.

### Invariants for AI Agents
1. **Never expose dormant/unfinished features**: If a feature is PLANNED or FOUNDATION, do not render placeholder buttons, dead links, fake forms, or mock authentication dialogs in consumer UI.
2. **Consult before planning**: Verify whether functionality already exists in the repository or whether dormant extension points are established.
3. **Completion compression rule**: When a feature reaches COMPLETED status, immediately compress its entry in this document to a concise summary with references to source files and test suites. Do not retain obsolete multi-page planning notes in the active backlog.

---

## 2. Current Implementation Priorities

- **P0**: Contextual hint overlap repair (clean flex-column separation of header, body, and action footer).
- **Primary Feature**: Short first-run onboarding (two-screen Welcome + Appearance with interactive miniature theme previews).
- **Reliability & Usability**: Rust-authoritative application evolution, CRUD data preservation, scheduler retention horizon, and dependency hygiene.

---

## 3. Future Onboarding Pipeline

### ONB-002: Account Sign-In Step
- **Status**: FOUNDATION
- **Purpose**: Optional user account authentication for cloud-assisted features and remote sync.
- **Existing Scaffolding**: Registered in `src/lib/onboarding/stages.ts` (`id: "account_sign_in"`, `enabled: false`).
- **Remaining Scope**: Connect to Supabase Auth Edge session exchange; store tokens in OS keychain; provide OAuth/email sign-in sheet.
- **Acceptance Criteria**: Consumer UI remains 100% offline-capable; sign-in is strictly optional; zero network requests during offline launch.
- **Security**: Hosted credentials and session tokens must never touch SQLite or plaintext configs.

### ONB-003: Continue Without Account Choice
- **Status**: PLANNED
- **Purpose**: Explicit user acknowledgement of local-only storage during a future account-enabled onboarding sequence.
- **Existing Scaffolding**: Coreside already operates local-first by default; dormant stage `offline_choice` in `src/lib/onboarding/stages.ts`.
- **Remaining Scope**: Provide explicit "Continue offline" prompt only if an account-creation screen is introduced in a future release.
- **UI Policy**: Do not expose a redundant "continue offline" button while Coreside is purely local-first.

### ONB-004: Account-Based Synchronization Setup
- **Status**: PLANNED
- **Purpose**: Optional user preference to synchronize workspace chats, tools, and wallpapers across multiple personal devices.
- **Existing Scaffolding**: Registered dormant stage `account_sync` in `src/lib/onboarding/stages.ts`.
- **Remaining Scope**: E2E encrypted sync protocol, device pairing, and conflict resolution rules.
- **UI Policy**: Strictly dormant until sync engine is implemented.

---

## 4. Future Account & Data Infrastructure

### ACC-001: Local-to-Account Workspace Migration
- **Status**: PLANNED
- **Purpose**: Migrate existing local SQLite turns, tools, records, and settings to an authenticated account without data loss.
- **Dependencies**: Server-side sync schema, Supabase Auth.
- **Security**: Local data must remain untouched if network migration fails; transactional rollback required.

### ACC-002: Encrypted Multi-Device Synchronization
- **Status**: PLANNED
- **Purpose**: Sync turn journal and application specifications between desktop instances.
- **Invariant**: Client-side encryption with user passphrase or OS keychain keys; zero-knowledge server storage.

---

## 5. Future Application Capabilities

### APP-001: Multi-View Persistent Routing for Generated Applications
- **Status**: FOUNDATION
- **Purpose**: Allow generated applications to declare multiple named views (e.g. List, Detail, Analytics, Settings) with deep navigation.
- **Existing Scaffolding**: `ApplicationSpec` and `view_mode` in `application_kernel`.
- **Remaining Scope**: Nested declarative routing and breadcrumb history inside `ToolCanvas`.

### APP-002: Advanced Visualization Capability Pack
- **Status**: PLANNED
- **Purpose**: Safe, declarative vector charts (bar, line, scatter) rendered without external CDN dependencies or raw scripts.
- **Security**: Pure SVG declarative renderer governed by Rust capability grant.

---

## 6. Completed Features (Compressed)

### ONB-001: Short Native Onboarding — COMPLETED
Two-screen first-run setup flow: Screen 1 (Welcome introduction with core concepts) and Screen 2 (Appearance selection). Moves the long 5-step tutorial to Help & learning without auto-launching on completion.
- **Source**: `src/components/onboarding/WelcomeDialog.tsx`, `src/stores/onboarding-store.ts`, `src/lib/onboarding/coordinator.ts`, `src-tauri/src/db/tutorial.rs`.
- **Tests**: `src/components/onboarding/TutorialOverlay.test.tsx`, `src/components/onboarding/ShortOnboarding.test.tsx`, `src/lib/onboarding/coordinator.test.ts`.

### HNT-001: Contextual Hint Layout Collision Fix — COMPLETED
Reworked `ContextualHint.tsx` and `.contextual-hint` CSS from an overlapping grid with absolute close button to a clean vertical flex structure (Header with title and close button, Body with description, Footer action row with Got it button). Guaranteed non-overlapping bounding boxes across all screen widths and text lengths.
- **Source**: `src/components/onboarding/ContextualHint.tsx`, `src/styles/global.css`.
- **Tests**: `src/components/onboarding/ContextualHint.test.tsx`, `src/lib/onboarding/contextual.test.ts`.

### THM-001: Interactive Light/Dark Application Previews — COMPLETED
Created `AppAppearancePreview.tsx` rendering a high-fidelity miniature Coreside desktop environment (macOS titlebar, sidebar navigation, chat messages, composer, and generated application panel) for Light and Dark modes. Uses local draft selection during onboarding so previewing does not prematurely commit durable settings.
- **Source**: `src/components/onboarding/AppAppearancePreview.tsx`, `src/components/onboarding/WelcomeDialog.tsx`, `src/styles/global.css`.
- **Tests**: `src/components/onboarding/ShortOnboarding.test.tsx`.

### STG-001: Architectural Onboarding Stage Registry & Dormant Gates — COMPLETED
Established typed stage registry in `src/lib/onboarding/stages.ts` supporting future onboarding steps (account sign-in, offline choice, sync) with strict dormant gates (`enabled: false`) preventing premature consumer UI exposure.
- **Source**: `src/lib/onboarding/stages.ts`.
- **Tests**: `src/lib/onboarding/stages.test.ts`.

### PERF-001: Native Startup Deferred Maintenance & Decoupled Frontend Hydration — COMPLETED
Offloaded SQLite turn journal compaction, attachment reconciliation, and disk cache pruning from synchronous startup to a deferred asynchronous background worker with safe Recovery-mode gating. Decoupled frontend active chat navigation from secondary provider catalog inspection and coalesced concurrent bootstrap requests into a shared in-flight promise.
- **Source**: `src-tauri/src/lib.rs`, `src/stores/app-store.ts`.
- **Tests**: `src/stores/app-store.test.ts`.

### CHAT-001: Bounded Fluid Text Presentation & Fast-Path Slice Stream Parsing — COMPLETED
Replaced unbounded character-by-character timer delay in buffered text delivery with bounded chunking (at most 10 frames, ~160ms total) with word-boundary snapping and immediate test/E2E emission. Added ASCII slice scanning fast path for unescaped partial JSON assistant message peeking to eliminate per-character unescaping loops during live streaming.
- **Source**: `src-tauri/src/commands/message_cmds.rs`, `src-tauri/src/ai/auto.rs`.
- **Tests**: `src-tauri/src/commands/message_cmds.rs` (`buffered_text_frame_tests`), `src-tauri/src/ai/auto.rs` (`peek_assistant_message_large_payload_slice_scan_performance`).


