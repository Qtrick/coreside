# Cloud Hosting Feasibility (Coreside)

**Product:** Coreside  
**Status:** Planning reference — not an implementation commitment  
**Last updated:** 2026-07-17  

This document answers: **which parts of today’s desktop Coreside can move to a cloud server and still work**, what must stay on-device, and what redesign is required.

It assumes the current architecture:

- Tauri desktop shell (React UI + Rust trusted core)
- Local SQLite
- OS keyring + `.env` credentials
- Exa Search for indexed discovery
- Crawl4AI as a **local** stdio sidecar for page inspection
- Media files on disk under app data
- Live wallpapers and Dock icons on the client

Consumer Settings currently hide **Web Search and Research** configuration (credentials / budgets / profiles). Those systems still run in-process for development via `.env` and protected Rust policy. A future cloud phase is expected to host research infrastructure remotely.

---

## 1. Executive summary

| Area | Cloud-hostable today? | Verdict |
| --- | --- | --- |
| Exa indexed search + cost accounting | **Yes** | Best first cloud candidate |
| AI provider calls (OpenRouter / Gemini / …) | **Yes** | Already HTTP; needs server secret store |
| Agent tool loop / citations / search profiles | **Yes** | Move with research API |
| Crawl4AI extraction | **Yes, as a dedicated worker** | Not serverless; needs browser fleet |
| Projects FTS / chat / tools DB | **Migratable** | Schema portable; sync model TBD |
| Media Library binaries | **Needs redesign** | Object storage + signed URLs |
| OS keyring / Dock / live wallpapers | **No (client-bound)** | Stay on device |
| Full “lift Tauri to the cloud” | **No** | Split into **Coreside Cloud API** + desktop/web clients |

**Recommended cloud shape (later):**

```text
Desktop / Web client
        │  authenticated API (no provider keys in the browser)
        ▼
Coreside Cloud API  (Rust or compatible service)
        ├── AI provider adapters
        ├── Exa Search + usage ledger + budgets
        ├── Research orchestrator (necessity, cache, handoff)
        ├── Crawl worker queue → Crawl4AI / Chromium pool
        ├── Postgres (or similar) for accounts, usage, citations metadata
        └── Object storage for imported media (optional sync)
```

Do **not** try to run the Tauri webview or OS keyring “in the cloud.” Host the **trusted research and AI core**; keep presentation, branding, Dock, and wallpapers local.

---

## 2. What “cloud-hostable” means here

A subsystem is cloud-hostable if it can:

1. Run on a remote machine without a user desktop session  
2. Accept authenticated requests from Coreside clients  
3. Keep secrets out of the webview and out of SQLite on the client  
4. Preserve current safety invariants (SSRF checks, robots, budgets, redaction)  
5. Return results the existing agent / citation / media pipelines can consume  

A subsystem is **not** cloud-hostable as-is if it depends on:

- Local filesystem paths the server cannot see  
- macOS/Windows keychain APIs  
- A long-lived GUI / Dock / wallpaper renderer  
- A per-user Python `.venv` next to the app binary with stdio IPC only  

---

## 3. Inventory by subsystem

### 3.1 Exa Search (indexed discovery)

**Current location:** `src-tauri/src/exa/`, hybrid orchestration in `research/hybrid.rs`  

**Cloud-hostable:** **Yes — high fit**

Why:

- Pure HTTPS to `https://api.exa.ai/search`
- Typed request/response, cost metadata (`costDollars`), retries, coalescing, and budgets are already centralized in Rust
- No browser, no GPU, no local disk requirement for the happy path
- Saver / Balanced / Thorough policies are server-enforceable (must not be client-trustable)

What to host:

| Piece | Cloud treatment |
| --- | --- |
| API key | Server secret (KMS / vault) or per-tenant BYOK sealed at rest |
| Search client | Same HTTP client logic as today |
| Usage ledger | Postgres table (port of `exa_usage_ledger`) |
| Monthly budget | Server-side enforcement; UI later as account settings |
| Query cache / coalescing | Redis or DB; shared across user sessions |
| Profiles (Saver default) | Account or org policy on server |

Client changes later:

- Desktop calls `POST /v1/research/search` instead of local Exa  
- Client never sees `EXA_API_KEY`  
- Optional: client still allows **user-supplied** Exa BYOK uploaded to the server vault (not SQLite)

Risks:

- Latency: one extra hop vs local Rust calling Exa  
- Multi-tenant isolation of cache fingerprints (must include tenant / project scope)  
- Do not expose raw Exa payloads with secrets in logs  

**Fit score: 9/10**

---

### 3.2 Crawl4AI (local page inspection)

**Current location:** `services/crawl4ai/` + `src-tauri/src/crawler/` (stdio NDJSON sidecar)

**Cloud-hostable:** **Yes as a worker service — not as the current sidecar**

Why it does not lift unchanged:

- Depends on a project-local Python venv and Playwright/Chromium  
- IPC is **stdio**, supervised by Tauri — not an HTTP admin API  
- Resource profiles (Eco / Balanced / Performance) assume one machine and one user  
- Cache lives under local data roots  

Why it can still be cloud-hosted:

- Work units are already URL-shaped: crawl URL(s), discover domain, extract Markdown / media refs  
- Compliance rules (robots on, no stealth, no proxies, SSRF re-check) are explicit and must remain  
- Horizontal scale is natural: queue jobs → pool of crawl containers  

Recommended cloud design:

```text
Research orchestrator
    │ enqueue CrawlJob { url, profile, limits, tenant }
    ▼
Crawl queue (SQS / NATS / Redis)
    ▼
Crawl workers (container image with Crawl4AI + Chromium)
    │ bounded concurrency, robots, domain rate limits
    ▼
Result store (Markdown excerpt, links, media candidates, cache key)
```

Hard constraints to preserve:

- Public HTTP(S) only; reject private/link-local ranges (same as today’s SSRF guard)  
- `check_robots_txt=True` always  
- No stealth / undetected browser / proxy modes  
- Per-tenant and global concurrency ceilings  
- Timeouts and cancel tokens (user Stop must cancel worker jobs)  
- Never pass Exa or AI API keys into the crawl worker environment  

Operational cost:

- Chromium workers are **memory- and CPU-heavy**  
- Cold start is slow; keep a warm pool  
- Prefer **not** to put Crawl4AI on the same tiny API node as Exa/AI routing  

**Fit score: 6/10 as-is · 8/10 after worker redesign**

---

### 3.3 Hybrid research orchestrator

**Current location:** `src-tauri/src/research/hybrid.rs` + search registry

**Cloud-hostable:** **Yes — should move with Exa + crawl workers**

Responsibilities that belong on the server:

1. Search necessity / seed classification (URL vs free-text)  
2. Profile caps (result count, crawl page count, refinements)  
3. Exa call + cache hit / coalesce  
4. Rank / select URLs  
5. Bounded Crawl4AI handoff  
6. Citation assembly  
7. Budget hard-stop before spend  

Client should receive:

- Normalized web / image / video results  
- Citations  
- Sanitized notices (“not configured”, “budget exceeded”)  
- Optional Action Log labels  

Do **not** let the desktop choose Deep Reasoning or raise result limits unilaterally once cloud-hosted.

**Fit score: 9/10**

---

### 3.4 AI providers and agent harness

**Current location:** `src-tauri/src/ai/`, tool loop, capability registry

**Cloud-hostable:** **Yes**

Already network-bound. Cloud options:

| Model | Description |
| --- | --- |
| **A. Server-proxied BYOK** | User key stored in vault; server calls providers |
| **B. Coreside-managed keys** | Product billing; user never handles Exa/LLM keys |
| **C. Hybrid** | Local BYOK for AI, cloud for research only |

Agent protocol (`tool_use` / `tool_change` / `settings_change`) can stay identical if the cloud returns the same JSON shapes the desktop already parses.

Longer-form work (multi-round tool_use) maps cleanly to server-side sessions with cancel and timeouts.

**Fit score: 8/10** (auth, tenancy, and streaming are the hard parts)

---

### 3.5 Credentials and secrets

**Current location:** OS keyring + `.env` fallback (`credentials/`, Exa resolve)

**Cloud-hostable:** **Not as keyring — replace with vault**

| Today | Cloud equivalent |
| --- | --- |
| macOS Keychain / Windows Credential Manager | KMS + encrypted secret store |
| `.env` on laptop | Server env / secret manager (ops only) |
| SQLite never stores keys | Same rule on server DB |
| Crawl sidecar never gets keys | Same rule for crawl workers |

Migration principle:

- Never auto-upload local keyring secrets without explicit user consent  
- Prefer **create new cloud credentials** over silently copying desktop keys  

**Fit score: 3/10 as-is · 9/10 with vault redesign**

---

### 3.6 SQLite application data

**Current location:** app-data SQLite (conversations, tools, projects, media metadata, usage ledger, settings)

**Cloud-hostable:** **Schema yes · product sync TBD**

Portable today:

- Conversations / messages  
- Tools + versions  
- Projects + FTS indexes (rebuild on server)  
- Exa usage ledger  
- Settings that are not device-specific  

Problematic without redesign:

- Absolute media file paths  
- Device-only wallpaper assignments  
- Dock icon preference  
- Local Crawl4AI cache directories  

Approaches:

1. **Cloud-only research account** — usage + research history on server; chats stay local  
2. **Full sync** — Postgres + conflict rules; media via object storage  
3. **Export/import** — offline bundles (already closer to exports design)  

**Fit score: 7/10 for metadata · 4/10 for full workspace sync without media redesign**

---

### 3.7 Media Library

**Current location:** `src-tauri/src/media/` (validation, magic bytes, local storage, thumbnails)

**Cloud-hostable:** **Partial**

What ports:

- Validation pipeline (MIME allowlists, size limits, rejection of HTML/executables)  
- Metadata + source attribution records  
- Approval-before-import policy  

What does not:

- Files living under local app data  
- Direct filesystem paths in wallpaper definitions  
- Thumbnail generation tied to local paths  

Cloud shape:

```text
Client selects candidate
  → upload or server-side fetch into object storage
  → validate
  → store object key + metadata
  → client downloads via signed URL when needed
```

Hotlinking remote URLs as persistent wallpapers must remain forbidden (same as today).

**Fit score: 5/10 as-is · 8/10 with object storage**

---

### 3.8 Wallpapers, themes, readability

**Current location:** React + CSS tokens + canvas presets; readability clamp in TS

**Cloud-hostable:** **No for rendering · Yes for preset catalogs**

Stay on client:

- Live wallpaper animation  
- Contrast clamping against local theme  
- Dock / in-app logos  

Optional cloud:

- Shared preset catalog / marketplace later (out of scope now)  
- Sync of chosen wallpaper **IDs** (not binary CSS/JS)  

**Fit score: 2/10 for runtime · 7/10 for preset metadata sync**

---

### 3.9 Automations

**Current location:** local scheduler while Coreside is open

**Cloud-hostable:** **Only with a redesign**

Today automations do not survive closed app sessions as a true always-on cloud cron. A cloud scheduler would need:

- Authenticated job definitions  
- No arbitrary shell/JS (keep current restrictions)  
- Clear user consent for background cloud actions  

**Fit score: 3/10 as-is**

---

### 3.10 Branding and Base Settings structure

**Protected** and product-owned. Cloud must not let tenants override Coreside logos or Base Settings structure. Same protected-resource IDs apply.

**Fit score: N/A (policy, not hosting)**

---

## 4. Suggested phased cloud rollout

### Phase 0 — Document and freeze boundaries (now)

- Keep Web Research settings out of consumer Base Settings  
- Continue local Exa via `EXA_API_KEY` / optional future BYOK  
- Keep Crawl4AI local for deep extraction  

### Phase 1 — Cloud Research API (Exa-first)

Host:

- Auth  
- Exa client + budgets + usage  
- Cache / coalesce  
- Normalized search results + citations  

Desktop:

- Feature flag: local vs cloud research  
- No Exa key in the client when cloud mode is on  

Crawl remains local **or** returns Exa-highlight-only answers until Phase 2.

### Phase 2 — Cloud Crawl Workers

- Queue + Crawl4AI containers  
- Same handoff bounds as Saver/Balanced/Thorough  
- Cancel propagation from desktop Stop  

### Phase 3 — Optional account sync

- Usage dashboards  
- Cross-device research history  
- Media object storage (if product wants it)  

### Explicit non-goals for early cloud

- Hosting the Tauri UI remotely  
- Replacing the desktop with a thin browser that holds API keys  
- Crawl stealth / proxy farms  
- Unlimited Deep Reasoning  
- Enterprise org billing complexity before consumer cloud research works  

---

## 5. Security and compliance when hosting

Carry forward from [SECURITY.md](./SECURITY.md) and crawler security docs:

| Invariant | Cloud note |
| --- | --- |
| No keys in SQLite / logs / Action Log | Same; plus no keys in client bundles |
| SSRF deny private ranges | Enforce on API **and** workers |
| Robots respected | Workers fail closed |
| Crawl never receives Exa/AI keys | Separate worker IAM |
| Budget hard-stop | Server authoritative |
| Agent cannot change budgets / profiles | Server allowlists |
| Tenant isolation | Cache keys, ledgers, crawl jobs scoped by account |
| Redaction | Centralize before any diagnostic export |

New cloud-only requirements:

- TLS everywhere  
- AuthN/AuthZ on every research endpoint  
- Rate limits per account (in addition to Exa’s)  
- Abuse monitoring for crawl fan-out  
- Data retention policy for cached page text  

---

## 6. Cost model implications

### Exa

- Already metered (`costDollars` / estimates)  
- Cloud can bill pass-through or include in subscription  
- Saver-default profiles remain the primary allowance protection  

### Crawl workers

- Dominated by **compute**, not Exa credits  
- Eco/Balanced/Performance become **fleet size / concurrency** knobs  
- Cache hits matter as much for Chromium time as for Exa dollars  

### AI tokens

- Separate from Exa; do not mix ledgers  
- Longer tool rounds increase provider cost — session caps stay mandatory  

---

## 7. API sketch (illustrative)

Not implemented. Shows how current Rust commands could map later:

| Desktop today (concept) | Cloud later |
| --- | --- |
| Local `web_search` / hybrid provider | `POST /v1/research/web` |
| Local `fetch_web_page` | `POST /v1/research/fetch` |
| Exa usage summary | `GET /v1/research/usage` |
| Exa budget | `GET/PUT /v1/research/budget` |
| Crawl cancel | `POST /v1/research/jobs/{id}/cancel` |

Response bodies should match existing frontend search/citation types so the React layer stays thin.

---

## 8. Decision matrix (quick reference)

| Component | Lift to cloud? | Blocker | First remediations |
| --- | --- | --- | --- |
| Exa client | Yes | Secrets location | Vault + tenant ledger |
| Hybrid orchestrator | Yes | None major | Extract service boundary |
| Search profiles / budgets | Yes | Must stay server-enforced | Remove client trust |
| Crawl4AI sidecar | No | stdio + local venv | Container workers + queue |
| AI adapters | Yes | Streaming + auth | Session API |
| Keyring | No | OS API | KMS |
| SQLite chats | Optional | Sync semantics | Start research-only cloud |
| Media files | Redesign | Paths | Object storage |
| Wallpapers / Dock | No | Client UX | Keep local |
| Automations | Redesign | App-open scheduler | Cloud cron later |

---

## 9. Honest gaps vs a production cloud

Even though Exa + orchestration are hostable, production still needs work that the desktop MVP does not fully provide:

- Multi-tenant auth and billing  
- Persistent distributed cache with privacy modes  
- Crawl fleet autoscaling and quarantine  
- End-to-end cancel across API → queue → worker  
- Observability without leaking queries/keys  
- Legal/ToS review for crawling from datacenter IPs  
- Clear UX when cloud research is unavailable offline  

Offline desktop mode should remain: local tools, local chats, optional local Crawl4AI if installed; cloud research degrades honestly (no fake SERP).

---

## 10. Recommendation

1. Treat **Exa + research orchestration + usage/budget** as the first cloud slice.  
2. Treat **Crawl4AI** as a **second service** (worker pool), not an in-process sidecar on the API node.  
3. Keep **credentials, Dock, wallpapers, and local tool UX** on the client.  
4. Defer full chat/media sync until research cloud is stable.  
5. Keep consumer Settings free of Web Research plumbing until the cloud account UI exists.

When implementation starts, add a concrete design doc (`docs/CLOUD_RESEARCH_API.md`) with auth choice, schema migrations, and SLOs — this file stays the feasibility map.

---

## Related documents

- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [EXA_INTEGRATION.md](./EXA_INTEGRATION.md)  
- [SEARCH_OPTIMIZATION.md](./SEARCH_OPTIMIZATION.md)  
- [CRAWL4AI_INTEGRATION.md](./CRAWL4AI_INTEGRATION.md)  
- [WEB_RESEARCH.md](./WEB_RESEARCH.md)  
- [SECURITY.md](./SECURITY.md)  
- [ROADMAP.md](./ROADMAP.md)  
