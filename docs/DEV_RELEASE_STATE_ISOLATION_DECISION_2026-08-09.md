# Dev / Release State Isolation Decision — 2026-08-09

## Question

Do `npm run dev` (debug adaptive `.app`) and release `Coreside.app` share meaningful state, and should development be isolated before public beta?

## Current facts (source-verified)

| Concern | Current behavior |
| --- | --- |
| Bundle ID | Both use `com.coreside.app` (debug + release) |
| SQLite / media / attachments / backups / crawler / recovery | `AppPaths` → `dirs::data_dir()/…/coreside` unless `CORESIDE_DATA_DIR` override |
| Provider keychain | Service `coreside.provider` |
| Search keychain | `coreside.search` |
| Concurrent processes | P0.4: fail-closed by default; `--allow-concurrent` / `CORESIDE_DEV_ALLOW_CONCURRENT=1` opt-in |
| E2E | Uses `CORESIDE_DB_PATH` / temp dirs — already isolated |
| Adaptive Dock | Driven by packaged `.app` + Assets.car; not by data-dir |

Changing `CFBundleIdentifier` alone does **not** isolate AppPaths or keychain services.

## Answers

1. **Share with release?** Preferences and BYOK keys are convenient to share during early consumer development; not safe as a permanent beta posture when migrations diverge.
2. **Never share?** Live user chats/projects during risky migration experiments; recovery/restore staging mid-flight; E2E seed DBs (already isolated).
3. **API keys in isolated dev?** Prefer OS keychain namespace `coreside.provider.dev` with optional copy-from-release on first launch, or explicit paste; keep `.env` as dev fallback only.
4. **Existing developer data?** Must not move silently. Any isolation needs an explicit one-time migration or “use shared data” opt-in.
5. **Migrations?** Shared DB means debug and release race the same schema — fail-closed concurrency mitigates; isolation removes the class.
6. **E2E?** Keep `CORESIDE_DB_PATH` / seed flags; do not point at user profile.
7. **Recovery/backup?** Belong with the active profile root; isolation implies separate backup trees.
8. **Hosted auth?** Session storage tied to webview/profile paths — verify before splitting; treat as high-risk.
9. **Adaptive debug bundle?** Can keep `com.coreside.app` if data isolation uses env/profile, not bundle-id hacks.
10. **Without changing bundle ID?** **Yes — preferred:** `CORESIDE_PROFILE=dev` → distinct `AppPaths` product root + keychain service suffix; release unchanged.

## Recommendation (pre-beta required design; low-risk implement later)

**Preferred model:** runtime profile boundary, not CFBundleIdentifier fork.

- Release (default): current paths + `coreside.provider` / `coreside.search`
- Development default: `CORESIDE_PROFILE=dev` → `…/coreside-dev/` + `coreside.provider.dev`
- Explicit `CORESIDE_PROFILE=shared` (or `CORESIDE_DEV_SHARE_DATA=1`) for developers who want today’s behavior
- Never auto-migrate user data; document copy/import tools later

## Implementation status (this pass)

**Not implemented** as a data-path split — too high-risk for silent credential/DB moves mid-P0.4.

**Implemented related safety:** concurrent Coreside fail-closed; branding exact-mirror; process-path robustness.

## Classification

`Integrated – Not Verified` → design complete; implementation deferred as **pre-beta required work**.
