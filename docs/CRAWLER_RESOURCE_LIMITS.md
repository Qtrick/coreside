# Crawler resource limits

Web Research uses named **resource profiles** so a desktop machine stays responsive. Profiles are stored in SQLite (`crawler_settings.webResearchResourceProfile`) and applied by Rust + the Python sidecar.

Default: **Balanced**.

## Profiles (UI)

| Profile | Typical concurrency | Pages / request (hint) | Memory soft threshold |
| --- | --- | --- | --- |
| Eco | 1 | 1 | ~62% |
| Balanced | 2 | 2 | ~70% |
| Performance | 3 | ≤4 | ~75% |

UI hints come from Rust `ResourceProfile`; **hard ceilings** are enforced in Python (`coreside_crawler/configs.py`).

## Hard ceilings (not AI-overridable)

Examples of fixed caps:

- Max active pages / pages per request / crawl depth
- Max extracted / Markdown character sizes
- Max media results; screenshots default off
- Idle browser shutdown (~3 minutes for Balanced)
- Page / job timeouts
- Hard memory block near ~90% system memory (fail with resource pressure)
- Minimum free disk (~200 MB) before starting a crawl

The agent and generated tools cannot raise these ceilings, disable robots, or enable stealth / proxies. Protected resource ids include `core.crawler.resource_limits` and related crawler keys.

## Setting the profile

- Settings → Web Research and Media → Resource profile
- Command: `set_web_research_resource_profile` with `{ profile: "eco" | "balanced" | "performance" }`

Unknown values normalize to `balanced`.
