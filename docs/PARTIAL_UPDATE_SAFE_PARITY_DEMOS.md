# Partial Update Safe Parity Demos

**Product:** Coreside  
**Access date:** 2026-07-18

| PU demo | Unsafe original | Coreside safe equivalent |
| --- | --- | --- |
| Interactive response | Raw HTML widgets | Declarative ToolRenderer surface |
| Structured form | HTML form → LLM | Trusted form components + context ledger submit |
| Game-like UI | Generated JS game | Local deterministic state + optional agent_request |
| Interface redesign | Replace whole HTML | Kernel ops / Customize mode |
| Code editing | CDN CodeMirror | Trusted code-editor pack (no auto-exec) |
| Charts | CDN d3 | Trusted chart pack |
| Update in place | Marker HTML patches | component.* ops + preservation |
| Preserve state | `preserve` attribute | PreservationPolicy + drafts + continuity |
| Silent update | Hidden HTML swap | `silent` + visibility policy (no hidden destructive) |

Automated coverage: `npm run test:preservation`, `npm run test:runtime-v2`, `npm run audit:partial-update-final-gaps`.
