# Partial Update File Analysis

Access date: 2026-07-18

Immutable reference: `.reference/partial-update/partialupdate-main/`

## `.env.example`

- Bytes: 1079
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `.gitignore`

- Bytes: 2097
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `.secrets.example`

- Bytes: 315
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `LICENSE`

- Bytes: 1068
- Purpose: MIT Copyright (c) 2026 Phil Holden.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `README.md`

- Bytes: 6033
- Purpose: Product overview, install, warnings, demos.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `migrations/0001_better_auth.sql`

- Bytes: 1730
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `migrations/0002_user_roles.sql`

- Bytes: 170
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `package-lock.json`

- Bytes: 58122
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `package.json`

- Bytes: 1289
- Purpose: Scripts/deps (better-auth, kysely, wrangler).
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `public/index.html`

- Bytes: 530
- Purpose: Entry redirect/bootstrap.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `scripts/dev-secure.sh`

- Bytes: 1300
- Purpose: Keychain-backed local secrets for dev.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `spec.md`

- Bytes: 2321
- Purpose: V2 protocol + events + wrangler notes.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/app.css`

- Bytes: 5771
- Purpose: Default app CSS.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/app.html`

- Bytes: 1358
- Purpose: Initial app shell with markers.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/auth/email.ts`

- Bytes: 1452
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/auth/profile.ts`

- Bytes: 877
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/auth/providers.ts`

- Bytes: 1722
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/auth/roles.ts`

- Bytes: 4029
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/auth/routes.ts`

- Bytes: 30631
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/auth/server.ts`

- Bytes: 3425
- Purpose: See source; cataloged in manifest.
- Coreside: deferred (local consumer). Attribution: none copied.

## `src/debug.ts`

- Bytes: 12868
- Purpose: Debug page rendering for LLM/visible history.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/env.ts`

- Bytes: 1191
- Purpose: Env typing for CF AI gateway and flags.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/getPrompt.ts`

- Bytes: 354
- Purpose: Injects APP_HTML/CSS + escape tokens into prompt.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/index.ts`

- Bytes: 100960
- Purpose: Main DO: routing, queue, rate limit, fork, replay, parse, LLM.
- Subsystems: DurableObject lifecycle; WebSocket accept/ping; chat create/load; prompt/form routes; LLM queue drain; rate limits; fork registry/snapshot; replay page; debug; UpdateStreamParser; delimiter coerce; client filtering; token injection.
- Coreside: split across `runtime_v2/*`, `ai/*`, `commands/message_cmds.rs` — no DO/WS copy.
- Direct reuse: **No**. Concepts only.

## `src/initialPrompt.md`

- Bytes: 10508
- Purpose: System prompt conventions for HTML/forms/silent/instances.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `src/page.css`

- Bytes: 2151
- Purpose: Page chrome CSS.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `tsconfig.json`

- Bytes: 414
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `worker-configuration.d.ts`

- Bytes: 514500
- Purpose: See source; cataloged in manifest.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

## `wrangler.jsonc`

- Bytes: 4258
- Purpose: CF worker/DO/D1 binding config.
- Adaptation: conceptual unless noted. Direct code reuse: **No**.

