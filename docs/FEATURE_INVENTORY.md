# Feature Inventory

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Legend:** `implemented` · `partial` · `missing` · `uncertain`  
Status from quick repo inspection (migrations 001–015, `src/` + `src-tauri/src/`), not a full QA pass.

## Archives (read-only extracts under `.reference/`)

| Archive | SHA-256 (2026-08-01) |
| --- | --- |
| Coreside Chat AI.zip | `5cdaa9f461d96322ca35de138e69ed04f4fc5ea57e9a75040de0abab08b09322` (refreshed; prior `044e7910…`) |
| Vendo main.zip | `516d00b41ca5051087b2e9838ef84bde6b42bad48d16df924fd35152f55e4a55` |

## Major features

| ID | Area | Status | Notes |
| --- | --- | --- | --- |
| `chat` | Chat, streaming, cancel, drafts | **implemented** | `src/components/chat/`, runtime_v2 queue |
| `projects` | Projects, FTS, isolation | **implemented** | migration 006, projects UI |
| `tools` | Personal tools / surfaces / versions | **implemented** | runtime_v2 + tool canvas |
| `kernel` | Application Kernel (manifest, packages, policy) | **implemented** | migrations 013–015 |
| `registered-actions` | Gateway, grants, approvals, breakers, audit | **implemented** | `registered_actions/` |
| `providers` | Gemini, OpenAI, Anthropic, OpenRouter BYOK | **implemented** | `src-tauri/src/ai/` |
| `ai-access-modes` | BYOK / local / env / hosted / unavailable | **partial** | Hosted adapter not shipped (`hosted_connected`) |
| `search` | Exa + Crawl4AI + budgets + citations | **partial** | Needs keys + sidecar; budgets present |
| `media` | Media Library, validation, import | **implemented** | migrations 007–008 |
| `wallpapers` | Templates / live / readability | **partial** | Out of scope for this public-beta docs phase; residual compositing risks remain |
| `automations` | Schedule, pause, app-bound grants | **implemented** | migrations 005/015 |
| `exports` | Tool/package export + redaction | **implemented** | `exports/` |
| `recovery` | Recovery Mode, LKG, execution gate | **partial** | Core gating present; full surface hiding uncertain |
| `multiwindow` | Secondary native tool windows | **partial** | Commands exist; manual journey K pending |
| `settings` | Base + Added Settings | **partial** | Category nav + search shipped; Privacy/Data/backup still thin |
| `action-log` | Off / Always / Intelligent | **implemented** | migrations 004, 011 |
| `runtime-v2` | Ops, transactions, events, branching | **implemented** | migration 012 |
| `continuity` | Drafts, preservation, patch scheduler | **implemented** | migration 014 |
| `mentions` | `@` tool references | **implemented** | `src/lib/mentions/` |
| `branding` | Protected logos / dock icons | **implemented** | protected resources |
| `e2e-desktop` | Packaged / WebDriver E2E matrix | **partial** | Scripts exist; full A–AB not recorded passing |
| `mcp` | External MCP door | **missing** | Rejected for consumer beta |
| `voice` | Voice input/output | **missing** | Rejected for consumer beta |
| `cloud-sync` | Accounts / sync / billing | **missing** | Out of scope |
| `enterprise` | Org admin / SSO / SCIM | **missing** | Deferred |

## Machine-readable

`reports/feature-inventory.json`
