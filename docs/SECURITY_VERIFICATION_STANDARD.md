# Security Verification Standard

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Internal verification target — **not a certification claim**

## Positioning

Coreside aims for an **OWASP ASVS Level 2–style** verification posture appropriate to a local-first desktop consumer app with BYOK credentials and a generative agent.

This document does **not** assert that Coreside is:

- ASVS certified  
- OWASP compliant as a marketed seal  
- Pen-tested or third-party audited in this phase  
- Ready for public beta solely because controls exist in code  

Evidence must be regenerable (`reports/security-findings.json`, `docs/SECURITY_ASSURANCE.md`, automated tests). Absence of evidence = open.

## Scope

| In scope | Out of scope (this standard) |
| --- | --- |
| Local BYOK desktop app | Enterprise SSO / org admin |
| Rust trust boundary + Tauri IPC | Claiming cloud hosted AI security before that track ships |
| Generative UI / registered actions | Unrestricted model JS execution (rejected product-wide) |
| SQLite local data + project isolation | Formal ISO/SOC paperwork |

## ASVS L2–style themes → Coreside checks

Map themes to concrete Coreside evidence. Prefer **fail closed** and **automated** where practical.

| Theme | Coreside verification focus | Primary evidence |
| --- | --- | --- |
| V1 Architecture | Protected core vs user-editable layer; single action gateway | `docs/ARCHITECTURE.md`, `protected_resources.rs`, registered-actions gateway |
| V2 Authentication | OS keyring for secrets; no keys in SQLite/UI | credential modules + tests; export redaction |
| V4 Access control | Project isolation; permission grants; Recovery Mode | FTS scope tests; permissions tests; recovery tests |
| V5 Input validation | Schema ops; URL/SSRF; media magic bytes | search safety, wallpaper validation, media import |
| V7 Error handling | Redacted errors; no raw serde in consumer paths | polish + sanitization reviews |
| V8 Data protection | Local data; export stripping; Action Log sanitization | export tests; Action Log policy |
| V9 Communication | Provider HTTP in Rust; TLS via trusted stack | AI adapters; no webview secret transport |
| V10 Malicious code | No model-authored JS; capability packs only | generative UI security model |
| V14 Config | Capabilities allowlists; agent cannot raise limits | Tauri capabilities; protected settings |

GenAI-specific overlays (OWASP GenAI Top 10): treat prompt injection and excessive agency via [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)—not via ASVS alone.

## NIST SSDF / CISA Secure by Design (lightweight)

| Practice | How Coreside applies it |
| --- | --- |
| Threat model before features | Docs in `docs/*SECURITY*`, generative UI model |
| Secure defaults | Action Log Off; budgets; fail-closed unknown actions |
| Review + test | `npm run test:permissions`, policy/recovery/AI-access tests; security review docs |
| No security theater | Do not claim certification; mark beta not ready without evidence |

## Verification workflow

1. Maintain control inventory (`docs/SECURITY.md`, `docs/SECURITY_ASSURANCE.md`).  
2. Run automated security-adjacent suites; record pass/fail honestly.  
3. Manual BYOK / export / protected-resource scenarios from release checklist.  
4. Update `reports/security-findings.json` when findings change.  
5. **Never** upgrade marketing language beyond evidence.

## Beta gate (security slice)

Public beta remains **blocked** until at least:

- Known credential/public-status regressions are fixed or explicitly waived with rationale  
- Registered-action Allow / Ask / Block paths have automated coverage (already partial)  
- No open P0 that lets generative UI reach secrets or arbitrary Tauri commands  
- This overhaul’s compositing/security docs exist and do not overclaim

Full E2E/packaged smoke for the overhaul: **not asserted here**.

## Related

- [SECURITY.md](./SECURITY.md)
- [SECURITY_ASSURANCE.md](./SECURITY_ASSURANCE.md)
- [AGENT_SECURITY_MODEL.md](./AGENT_SECURITY_MODEL.md)
- [GENERATIVE_UI_SECURITY_MODEL.md](./GENERATIVE_UI_SECURITY_MODEL.md)
