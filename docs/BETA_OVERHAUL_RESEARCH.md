# Beta Overhaul Research

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Research complete for planning; beta **not ready**  
**Archives (gitignored extracts):**

| Archive | SHA-256 | Extract path |
| --- | --- | --- |
| Coreside Chat AI.zip | `5cdaa9f461d96322ca35de138e69ed04f4fc5ea57e9a75040de0abab08b09322` (refreshed; prior `044e7910…`) | `.reference/coreside-uploaded-2026-08-01` |
| Vendo main.zip | `516d00b41ca5051087b2e9838ef84bde6b42bad48d16df924fd35152f55e4a55` | `.reference/vendo-uploaded-2026-08-01` |

Manifests: `reports/coreside-uploaded-manifest.json`, `reports/vendo-uploaded-manifest.json`.

## Purpose

Define the evidence base for a consumer beta overhaul: surface compositing, wallpaper assurance, settings IA, and security verification—without claiming certification or full release readiness.

## Sources reviewed

| Source | Role for Coreside | URL / note | Access date |
| --- | --- | --- | --- |
| Tauri 2 | Desktop shell, capabilities, IPC trust boundary | https://v2.tauri.app/ — repo uses `@tauri-apps/*` ^2.5 / `tauri = "2"` | 2026-08-01 |
| OWASP ASVS 5.0 | Verification requirement catalog; **target style: Level 2** (not a claim of certification) | https://owasp.org/www-project-application-security-verification-standard/ | 2026-08-01 |
| OWASP Top 10:2025 | Web/app risk orientation for IPC, authn of actions, injection | https://owasp.org/Top10/ | 2026-08-01 |
| OWASP Top 10 for LLM / GenAI Apps | Prompt injection, excessive agency, sensitive info disclosure | https://genai.owasp.org/ | 2026-08-01 |
| NIST SSDF (SP 800-218) | Secure development practices (threat model, review, tests) | https://csrc.nist.gov/pubs/sp/800/218/final | 2026-08-01 |
| CISA Secure by Design | Default-secure product posture; memory-safe where practical; no “security theater” claims | https://www.cisa.gov/securebydesign | 2026-08-01 |
| SQLite | Local persistence; migrations; project isolation | https://www.sqlite.org/ — bundled via `rusqlite` | 2026-08-01 |
| WCAG 2.2 | Contrast, focus, non-text contrast; wallpaper readability | https://www.w3.org/TR/WCAG22/ | 2026-08-01 |
| Existing Coreside docs | Architecture, security, wallpapers, generative UI | `docs/SECURITY.md`, `docs/WALLPAPER_COMPOSITING_RESEARCH.md`, `docs/GENERATIVE_UI_SECURITY_MODEL.md` | 2026-08-01 |
| Uploaded Coreside snapshot | Prior product tree for diff/continuity | extract above (619 files) | 2026-08-01 |
| Uploaded Vendo snapshot | Host-visual / agent-host lessons only—**do not copy branding or unrestricted execution** | extract above (2850 files); `docs/VENDO_REFERENCE_AND_ADOPTION_AUDIT.md` | 2026-08-01 |

## Findings

1. **Wallpaper still loses to nested opaque fills.** Panel alpha tokens exist (`src/lib/interface-transparency.ts`), but child rules using opaque `var(--surface)` or tint-onto-`--surface` recreate occlusion. Root cause and fix path: [WALLPAPER_ASSURANCE.md](./WALLPAPER_ASSURANCE.md).
2. **Compositing needs named depth categories**, not ad-hoc `opacity` on parents. Model: [SURFACE_COMPOSITING_MODEL.md](./SURFACE_COMPOSITING_MODEL.md).
3. **Settings IA is flat and mixed.** Current Base Settings mixes Appearance, AI providers, Agent Behavior, Data, Accessibility, Recovery, Runtime Permissions, About; Wallpapers sit under Added Settings → Templates. Proposed consumer categories: [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md).
4. **Security enforcement is real but unverified as a product claim.** Registered-action gateway uses Allow / RequireApproval / Block (`application_kernel/registered_actions/policy.rs`). Verification standard must stay ASVS-L2-**style** without false certification: [SECURITY_VERIFICATION_STANDARD.md](./SECURITY_VERIFICATION_STANDARD.md), [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md).
5. **Beta readiness is not met.** Local-BYOK evidence may show partial offline gates; this overhaul does **not** assert that full E2E or packaged smoke for the overhaul work has passed. Treat public beta as **blocked** until compositing proof, settings IA, and security verification evidence land.

## Decisions

| ID | Decision |
| --- | --- |
| D1 | Interface transparency range remains **0–60%** (default 20%); never parent-element `opacity` for panels. |
| D2 | Depth categories + alpha mapping are normative for protected chrome; inventory remaining `var(--surface)` risk in `reports/background-rule-inventory.json`. |
| D3 | Wallpaper “None” clears schema JSON; never persist `{kind:"none"}` into schema `wallpaperJson`. |
| D4 | Settings copy stays consumer-first; developer/technical language lives under Advanced (or docs), not primary labels. |
| D5 | Security target: **ASVS Level 2–style coverage** for local desktop BYOK; **no** “ASVS certified / OWASP compliant” marketing claims. |
| D6 | Agent side effects choke at Rust gateways (tool loop + registered actions); GenAI Top 10 mapped as injection/agency boundaries, not as a free-form shell. |
| D7 | Vendo is reference-only for host polish lessons; Partial Update unrestricted execution remains rejected. |

## Non-claims

- Not WCAG certified.
- Not ASVS certified / audited by a third party in this phase.
- Not ready for public beta as of 2026-08-01 overhaul kickoff.
- Full packaged smoke + complete desktop E2E matrix for this overhaul: **not claimed passed**.

## Related

- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) — beta overhaul phases (top section)
- [SURFACE_COMPOSITING_MODEL.md](./SURFACE_COMPOSITING_MODEL.md)
- [WALLPAPER_ASSURANCE.md](./WALLPAPER_ASSURANCE.md)
- [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md)
- [SECURITY_VERIFICATION_STANDARD.md](./SECURITY_VERIFICATION_STANDARD.md)
- [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)
