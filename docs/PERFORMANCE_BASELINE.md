# Performance Baseline

**Date:** 2026-08-01  
**Commit:** measured on working tree after cleansing changes  
**Environment:** macOS 26.5.2 arm64, Node v24.17.0

This baseline records what can be measured without a packaged desktop build or
interactive GUI. Numbers come from actual command output.

## Web bundle (`npm run build:web`)

| Asset | Size | Gzip |
| --- | --- | --- |
| `dist/assets/index-*.js` | 741.50 kB | 209.07 kB |
| `dist/assets/index-*.css` | 55.51 kB | 9.91 kB |
| Dock PNGs (2) | ~174 kB combined | — |
| `dist/index.html` | 0.78 kB | 0.42 kB |

Vite warns that the main JS chunk exceeds 500 kB. Primary concentration:
`src/lib/tauri.ts` (static import from most surfaces). Deferred: code-split
the mock bridge out of production builds.

## Automated gate timings (order-of-magnitude)

| Gate | Observed |
| --- | --- |
| `npm run typecheck` | ~2–4 s |
| `npm run lint` | ~2 s |
| `npm test` (78) | ~1 s |
| `cargo test` (~311) | ~2 s once compiled |
| `db::migration_fixtures` (10) | ~0.25 s |
| `npm run build:web` | ~1.1 s vite build after tsc |

## Not measured (blocked)

| Metric | Reason |
| --- | --- |
| Cold / warm app startup | Needs packaged or `tauri dev` GUI session |
| Live wallpaper idle CPU | Needs running desktop app |
| Packaged `.app` / `.dmg` size | `npm run build` not run this pass |
| Conversation switch latency | Needs instrumented desktop session |
| Secondary window open time | Needs desktop |

## Optimizations already applied this phase

- Approval refresh: 4 s polling → event-driven `runtime-approvals-changed`
- Sidebar / Settings / ToolCanvas: full-store Zustand → `useShallow` selectors
- `get_recent_messages`: SQL `LIMIT` instead of load-all truncate
- Event-bus idempotency set: bounded FIFO
- Snapshot retention: oldest pruned at `MAX_SNAPSHOTS_PER_CONVERSATION`
- Branch creation: rejects at `MAX_BRANCH_DEPTH`

## Next measurements to capture

1. Packaged build size after `npm run build`
2. Instruments/time profiler on chat switch with 500+ messages
3. Confirm live wallpaper pauses on blur/hidden
