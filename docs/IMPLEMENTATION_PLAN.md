# Implementation Plan — Exa + Crawl4AI Hybrid, Wallpapers Templates, Readability

**Product:** Coreside  
**Status:** Hybrid MVP shipped — see Remaining below  
**Last updated:** 2026-07-17

## Current-state findings (resolved)

- Exa + Crawl4AI hybrid via `HybridSearchProvider` ([`research/hybrid.rs`](../src-tauri/src/research/hybrid.rs)).
- Brave removed from active path; legacy citations remain readable.
- Wallpaper “None stays selected” fixed via `activeCanvasPresetId` from `wallpaperJson`.
- Wallpapers under Added Settings → Templates (not Base Settings).
- Migration `010_exa_wallpapers.sql`; `EXA_API_KEY=` in `.env` / `.env.example`.

## Architecture

```
Necessity → cache/coalesce → Exa Search (discovery+highlights)
  → rank/select → Crawl4AI (≤N pages, profile-bounded) → citations + usage ledger
```

Direct URL → Crawl4AI only (no Exa).  
No Exa key → honest setup / local-only (no fake SERP).  
**Never** route default search through Exa Agent.

## Phases

1. Exa research docs ✅  
2. Rust `exa/` client + credentials + usage/budget + migration ✅  
3. Hybrid orchestrator + registry + agent prompts ✅  
4. Settings UI (Exa + profiles + usage) ✅  
5. Wallpaper Templates + selection fix + Live badges ✅ (Apply-only)  
6. Readability MVP (tokens + contrast helpers) ✅  
7. Docs + verify + code-reviewer-editor loops ✅  

## Remaining (next iteration)

- Deeper necessity classifier + Thorough refinement loop  
- Persistent Exa query-cache table + query-history privacy toggle  
- Full wallpaper create/delete/hide/preview-vs-apply  
- Full readability (gradient/image/video sampling)  
- Exa HTTP cancellation tied to agent cancel token  

## Defaults

Saver profile; Exa Contents fallback off; Deep Reasoning never silent; result max 10; local budget optional and independent of Exa balance.
