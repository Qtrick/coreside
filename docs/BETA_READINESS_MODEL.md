# Beta Readiness Model

**Product:** Coreside  
**Phase:** RC3.4  
**Source of truth:** `reports/readiness-ladder.json` (generate with `npm run audit:readiness`)

This model classifies **evidence**, not marketing. Do not invent Desktop Verified, Packaged Verified, Human Accepted, or Public Beta without matching artifacts.

## Evidence states

| State | Meaning |
| --- | --- |
| **Absent** | No meaningful implementation or stub for the capability. |
| **Scaffolded** | Spec, types, stubs, or partial wiring exist; not production-complete. |
| **Integrated – Not Verified** | Present in the running app path; no automated proof recorded. |
| **Unit Verified** | Automated unit/integration tests pass for the claimed slice. Not desktop proof. |
| **Desktop Verified** | Recorded desktop/dev E2E journey pass on a named commit. |
| **Packaged Verified** | Recorded packaged/smoke proof on a named commit. |
| **Cross-Platform Verified** | Same claim proven on more than one target OS/arch. |
| **Human Accepted** | Signed human checklist for that claim (`docs/HUMAN_ACCEPTANCE_CHECKLIST.md`). |
| **Blocked by External Prerequisite** | Cannot advance until an external dependency is available. |
| **Deliberately Deferred** | Explicitly postponed; not a silent gap. |
| **Rejected by Product Architecture** | Intentionally out of scope (e.g. raw HTML/JS/CDN/iframe execution). |

Higher states subsume lower implementation states only when evidence is present. Unit Verified never implies Desktop Verified.

## Readiness ladder (overall)

Ordered from weakest to strongest release posture:

1. **Development Build** — Local dirty/clean worktree; features may be unit-proven only.
2. **Internal Alpha** — Usable for trusted internal operators; known gaps documented.
3. **Private Alpha** — Limited external invitees; tracks separated (local BYOK vs hosted).
4. **Automated Public-Beta Candidate** — Offline gates + desktop/packaged automation green; human still unsigned.
5. **Human-Approved Public Beta** — Automated candidate **plus** signed human acceptance.
6. **Consumer Launch Candidate** — Public-beta bar met across required platforms and residual P0s closed.

Tracks stay separate:

| Track | Typical current posture |
| --- | --- |
| Local-first BYOK | Development Build (may approach Internal Alpha when offline gates are green and dirty=false) |
| Hosted Coreside AI | Development Build (often Deliberately Deferred relative to local beta) |

## Honesty rules

- Spec-only E2E ≠ Desktop Verified.
- Dirty trees cannot claim public-beta ladder rungs.
- Hosted AI blockers must not fail local BYOK verdicts (and vice versa).
- `npm run audit:readiness` records classification; it does not upgrade evidence by itself.

## Related

- Checklist template: [HUMAN_ACCEPTANCE_CHECKLIST.md](./HUMAN_ACCEPTANCE_CHECKLIST.md)
- Release narrative: [RELEASE_READINESS.md](./RELEASE_READINESS.md)
- Known gaps: [KNOWN_ISSUES.md](./KNOWN_ISSUES.md)
