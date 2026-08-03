# Partial Update File Analysis (2026 Re-Audit)

**Product:** Coreside  
**Access date:** 2026-08-03  
**Reference extract:** `.reference/partial-update/partialupdate-main/`  
**Archive SHA-256:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607` (byte-identical to July 2026)  
**Active Coreside commit:** `41fd4595db420eac8b1c5cc30723f1dd377608f4` (dirty)

Machine-readable mirror: `reports/partial-update-file-manifest-current.json`.

**Policy:** Direct code reuse into the protected webview is **No** for every file. Adaptation is conceptual / secure-replacement / deferred / rejected as noted. July `PARTIAL_UPDATE_FILE_ANALYSIS.md` catalog remains historically useful; Coreside mapping rows below reflect **2026-08-03 evidence**.

---

## Root / project

### `.env.example` (1079 B)

- **Purpose:** Cloudflare account, AI Gateway, auth, rate-limit env examples.
- **Adaptation:** conceptual only. **Direct reuse:** No.
- **Coreside:** OS-secure BYOK + AppConfig; Hosted AI **Not ready**. Do not copy plaintext gateway secrets into app storage.

### `.gitignore` (2097 B)

- **Purpose:** Ignore secrets, `node_modules`, wrangler state.
- **Adaptation:** conceptual. **Direct reuse:** No.

### `.secrets.example` (315 B)

- **Purpose:** Deploy secrets file shape.
- **Adaptation:** conceptual. **Direct reuse:** No.
- **Coreside:** Never store provider keys in SQLite / exports / diagnostics.

### `LICENSE` (1068 B)

- **Purpose:** MIT Copyright (c) 2026 Phil Holden.
- **Adaptation:** attribution. **Direct reuse:** No (license text may be cited in notices).
- **Coreside:** See `THIRD_PARTY_NOTICES.md`.

### `README.md` (6033 B)

- **Purpose:** Product demos — HTML UI, forms, multiuser games, CDN libraries, Wikipedia full-page rewrite, warnings about cost loops.
- **Adaptation:** product-insight only. **Direct reuse:** No.
- **Security:** Documents CDN/JS injection and inference/Worker burn loops — **rejected** for Coreside webview.
- **Coreside mapping:** Persist generative state via Runtime V2; reject HTML/JS/CDN/iframe forms.

### `package.json` (1289 B) / `package-lock.json` (58122 B)

- **Purpose:** Wrangler scripts; better-auth / kysely Worker deps.
- **Adaptation:** none. **Direct reuse:** No.
- **Coreside:** Separate Node/Rust lockfiles; do not merge Worker dependency graph.

### `tsconfig.json` (414 B)

- **Purpose:** TS options for Worker. **Direct reuse:** No.

### `wrangler.jsonc` (4258 B)

- **Purpose:** Worker name `partialupdate`, DO bindings, D1, vars.
- **Adaptation:** rejected_product_direction (CF Worker runtime). **Direct reuse:** No.

### `worker-configuration.d.ts` (514500 B)

- **Purpose:** Generated Cloudflare Worker type stubs (largest file).
- **Adaptation:** none. **Direct reuse:** No.

### `public/index.html` (530 B)

- **Purpose:** Minimal public entry. **Direct reuse:** No.

### `scripts/dev-secure.sh` (1300 B)

- **Purpose:** Keychain-backed local `.dev.vars` bootstrap.
- **Adaptation:** conceptual for secure local secrets. **Direct reuse:** No.
- **Coreside:** Prefer OS credential storage; production `.env` policy is debug-gated.

### `spec.md` (2321 B)

- **Purpose:** V2 delimiter protocol (`SPLIT_MESSAGE`, `SERVER_PROPS`, `CLIENT_PROPS`, `BODY`), client `subscribe` API, wrangler notes.
- **Adaptation:** secure_adaptation_required → **JSON schema v2 operations**, not delimiter HTML.
- **Direct reuse:** No.
- **Coreside:** `runtime_v2/operations.rs`, response schema; audience include/exclude deferred.

---

## Migrations

### `migrations/0001_better_auth.sql` (1730 B)

- **Purpose:** D1 user/session/account/verification tables.
- **Adaptation:** deferred_enterprise. **Direct reuse:** No.
- **Coreside:** Local SQLite profile migrations only.

### `migrations/0002_user_roles.sql` (170 B)

- **Purpose:** `user.role` default `view`; first user admin.
- **Adaptation:** deferred_enterprise. **Direct reuse:** No.

---

## `src/` application

### `src/index.ts` (100960 B) — primary runtime

- **Purpose:** `PartialUpdate` Durable Object: hibernatable WebSockets, form routes, LLM queue (limit 5), fork index, replay pages, rate-limit permits, stream parse/apply, chat history storage.
- **Adaptation:** secure_adaptation_required for product behaviors; **reject** HTML apply, iframe forms, DO multiuser.
- **Direct reuse:** No.
- **Coreside mappings:**
  | PU concern | Coreside target | 2026-08-03 status |
  | --- | --- | --- |
  | Stream apply | `AiProvider` stream + `streaming.rs` | **True streaming ABSENT**; `emit_text_fluidly` fake |
  | Form POST | `submitToAgent` + ledger→prompt | Ledger write **without** prompt injection |
  | Queue | `runtime_v2/queue.rs` | Backend present; **UI disconnected** |
  | Fork | `runtime_v2/branch.rs` | Backend present; **UI disconnected** |
  | Replay | transactions + player UI | APIs present; **UI disconnected** |
  | Rate limits | budgets / limits | Different model; partial |

### `src/initialPrompt.md` (10508 B)

- **Purpose:** Model instructions: escape string, APP_HTML, markers, HTML forms, CDN libraries, multiuser colors/privacy, silent updates, instances.
- **Adaptation:** rewrite as **protected Coreside agent instructions** for declarative ops — never instruct HTML/JS/CDN.
- **Direct reuse:** No.
- **Security:** Primary source of unsafe generative contract. **Rejected** execution semantics.

### `src/getPrompt.ts` (354 B)

- **Purpose:** Compose system prompt injecting `app.html` / `app.css` / `initialPrompt.md` plus client/chat/fork IDs.
- **Adaptation:** conceptual (prompt composition). **Direct reuse:** No.
- **Coreside:** `build_agent_prompt_with_references` — must later include context ledger entries (currently missing).

### `src/app.html` (1358 B)

- **Purpose:** Initial shell with `<?marker name="…">` zones for layout/chat/style overrides.
- **Adaptation:** map to stable surface zones / Tool Canvas / inline surfaces — **not** raw marker HTML.
- **Direct reuse:** No.

### `src/app.css` (5771 B) / `src/page.css` (2151 B)

- **Purpose:** Default card chat styling; light/dark page variables.
- **Adaptation:** design-language inspiration only; follow Coreside tokens. **Direct reuse:** No.

### `src/debug.ts` (12868 B)

- **Purpose:** Raw/pretty LLM history debug HTML views; clear-history helpers.
- **Adaptation:** Developer Mode **redacted** inspector. **Direct reuse:** No.
- **Coreside:** Developer Mode toggle exists; **inspector UI disconnected**.

### `src/env.ts` (1191 B)

- **Purpose:** Worker env typing (AI Gateway, auth, rate limits).
- **Adaptation:** none for CF types. **Direct reuse:** No.

---

## `src/auth/` (deferred enterprise)

| File | Bytes | Purpose | Coreside |
| --- | ---: | --- | --- |
| `email.ts` | 1452 | Verification email | Deferred |
| `profile.ts` | 877 | Consent upsert | Deferred |
| `providers.ts` | 1722 | Provider enablement | Deferred |
| `roles.ts` | 4029 | admin/dev/chat/view/blocked permissions | Deferred |
| `routes.ts` | 30631 | Auth HTTP/UI routes | Deferred |
| `server.ts` | 3425 | betterAuth construction | Deferred |

**Direct reuse:** No. Consumer Coreside remains local-first without cloud OAuth in current scope.

---

## Systems covered by file set

1. Protocol / streaming / forms / queue / fork / replay / rate limits → `src/index.ts`, `spec.md`, `src/initialPrompt.md`
2. Markers / shell → `src/app.html`, CSS
3. Prompt composition → `src/getPrompt.ts`, `src/initialPrompt.md`
4. Debug → `src/debug.ts`
5. Auth / D1 → `src/auth/*`, migrations
6. Infra → `wrangler.jsonc`, `package.json`, env examples

## Attribution

Partial Update is MIT, Copyright © 2026 Phil Holden. Coreside studies it for generative-UI product lessons and does **not** copy unrestricted HTML/JS/CDN execution into the protected webview.
