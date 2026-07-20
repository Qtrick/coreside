# Performance Assurance

**Product:** Coreside v0.1.0  
**Last updated:** 2026-07-19

## Baseline status

**Performance baselines are NOT YET MEASURED.**

This document intentionally contains **no** startup times, memory figures, frame rates, or search latency numbers. Any future metrics must come from recorded runs on named hardware with commit SHA and build type (dev vs packaged).

## Documented limits (not measurements)

See `docs/PERFORMANCE_LIMITS.md` for architectural caps (patch queue, search budgets, crawl limits, etc.). Those are **policy bounds**, not benchmark results.

## Areas requiring future measurement

| Area | Suggested metric | Tooling (not yet run) |
| --- | --- | --- |
| Cold start | Time to interactive chat | Instruments / manual stopwatch on packaged macOS |
| Chat streaming | Tokens/s perceived latency | Provider-dependent; log in manual session |
| SQLite migration 001→014 | Upgrade duration on large DB | Fixture DB + scripted upgrade |
| Media import | Peak memory on large video | Activity Monitor during manual test |
| Wallpaper video | CPU/GPU with live wallpaper | Activity Monitor |
| Crawl4AI page fetch | P95 latency per page | `crawl4ai:stats` + manual crawl |
| Exa search | Requests/month vs budget | In-app search usage UI |
| Secondary window open | Window creation latency | Manual / WebDriver future |

## Existing non-performance tests

Rust tests include backpressure (`patch_scheduler::tests::backpressure_limit`) — logic correctness, not throughput benchmarking.

`docs/ROADMAP.md` mentions "Local provider conformance benchmark runs (record real measurements only)" — **not done**.

## Release stance

- **Internal alpha:** Performance regression hunting is informal.
- **Public beta:** Requires at least cold-start and chat-stream baselines on macOS packaged build, documented with hardware spec.

## Do not claim

- "Fast startup" without measurement
- "Low memory" without profiling
- "60fps wallpapers" without frame timing evidence
