# Partial Update Final Gap Research

**Product:** Coreside  
**Access date:** 2026-07-18  
**Purpose:** Research for identity preservation, patch scheduling, navigation continuity, direct manipulation, optimistic reconciliation, context ledgers, and provider conformance — adapted safely into Coreside’s declarative runtime.

## Sources reviewed

| Source | URL | Reviewed |
| --- | --- | --- |
| Partial Update (philholden) | https://github.com/philholden/partialupdate | 2026-07-18 |
| WICG Declarative Partial Updates | https://github.com/WICG/declarative-partial-updates | 2026-07-18 |
| Patching explainer | https://github.com/WICG/declarative-partial-updates/blob/main/patching-explainer.md | 2026-07-18 |
| Preserve explainer | https://github.com/WICG/declarative-partial-updates/blob/main/preserve-explainer.md | 2026-07-18 |
| Route-matching explainer | https://github.com/WICG/declarative-partial-updates/blob/main/route-matching-explainer.md | 2026-07-18 |
| Dynamic markup explainer | https://github.com/WICG/declarative-partial-updates/blob/main/dynamic-markup-revamped-explainer.md | referenced via Chrome blog |
| Chrome blog | https://developer.chrome.com/blog/declarative-partial-updates | 2026-07-18 |
| Prior Coreside PU docs | `docs/PARTIAL_UPDATE_*.md`, `docs/RUNTIME_V2_COMPLETION_AUDIT.md` | 2026-07-18 |
| Supplied video | https://www.youtube.com/watch?v=f39MnczcJZA | prior analysis in `PARTIAL_UPDATE_VIDEO_ANALYSIS.md` |

## Relevant findings

### Preserve (WICG)
- `preserve` attribute as an island key: matching keys keep live instances (iframe/video) while surrounding content replaces.
- Prefer islands over full morph — overwrite by default, preserve by exception.
- Prior art: HTMX `hx-preserve`, Astro islands.

### Patching (WICG)
- Named markers / start–end ranges; out-of-order and interleaved patches.
- Nested targets; template `for=` binding.
- Target-not-found leaves template for diagnostics.
- Script-initiated streaming (`streamAppendHTMLUnsafe`) is a separate browser concern.
- Security: sanitizer / Trusted Types; same-parent restrictions for templates.

### Routes (WICG)
- Same-document navigation with route matching, loading UI, history, progressive enhancement.
- Unsupported clients need fallbacks.

### Partial Update product (philholden)
- LLM emits HTML/CSS/JS/CDN — **unsafe for Coreside**.
- Valuable behaviors: interactive forms → LLM, in-place redesign, multiplayer (deferred), streaming islands.

### Chrome blog (2026-05-19)
- Experimental Chrome 148+ flags; polyfills exist.
- Emphasizes out-of-order streaming + safer HTML insertion APIs.
- Future: contentrevision to avoid overwriting unchanged islands.

## Already covered in Coreside

- Stable surface / instance / component IDs (`runtime_v2`)
- Fine-grained ops + transactions + revision conflicts
- Application Kernel, manifests, packages, recovery
- Agent queue, EventBus hydrate, NDJSON harvest
- Proposal approval UI, multiwindow conflict banner
- Chat draft/scroll in-memory; tool state persistence

## Not adequately covered (this phase)

- Keyed preservation policies beyond React keys
- Draft protection across agent patches
- Focus / caret / selection / media continuity
- Patch scheduler (deps, priority, backpressure, supersession)
- Generated-app route history + same-route no-op
- Customize / direct manipulation
- Optimistic local transactions with rollback
- Context ledger (model-only vs visible)
- Provider conformance profiles + buffered fallback
- Surface state hydration for inline surfaces
- Suspension of offscreen expensive surfaces

## Coreside adaptation decisions

| Browser / PU concept | Coreside adaptation |
| --- | --- |
| `preserve` attribute | Trusted `PreservationPolicy` + `preservationKey` on components |
| Marker / template HTML patches | Structured `AppOperation` via Patch Scheduler → Kernel |
| Out-of-order streaming HTML | Scheduler dependency graph + sequence numbers |
| Same-document navigation | Namespaced generated routes in manifest + `application_route_state` |
| Unrestricted HTML/JS/CDN | **Rejected** — declarative packs only |
| Multiuser collaboration | **Deferred** (enterprise / future) |
| Contentrevision skip | `baseRevision` + supersession + compatibility checks |

## Security differences

Coreside must never:
- Apply model HTML/JS
- Let the model invent preservation policies or raise patch priority
- Silently discard drafts
- Leak model-only context across projects
- Auto-resume media/mic after restart

## Exact implementation decisions

1. Migration `014_continuity_scheduler.sql` for drafts, preservation metadata, scheduler, routes, ledger, conformance, continuity, manual provenance.
2. Patch Scheduler sits between stream validation and Kernel apply.
3. Frontend owns live focus/scroll/media capture; Rust persists continuity snapshots and drafts.
4. Customize mode emits the same `AppOperation` protocol with `sourceType: direct_manipulation`.
5. Provider profiles are static seeded records + optional local benchmarks — never fabricated success metrics.
