# Crawler security

Web Research security is layered: Rust validates outbound targets; the Python sidecar re-checks and runs with locked-down browser configs.

## SSRF

Before crawl / fetch:

- Allow only `http` / `https`
- Reject embedded credentials, localhost, private / link-local / loopback ranges
- Resolve DNS and reject private resolved IPs (rebinding-aware checks where implemented)
- Cap redirects and response sizes on HTTP fetch helpers

Sidecar `coreside_crawler/security.py` mirrors URL policy and raises `ssrf_blocked` on violation.

## Process isolation

- Sidecar speaks **stdio NDJSON only** — no Crawl4AI HTTP admin port exposed by Coreside
- One supervised child process; unexpected exit marks jobs failed and can restart with backoff
- Cancel maps to protocol `cancel`; force-kill if acknowledgement is missing
- AI cannot inject arbitrary browser configs, raise resource ceilings, or disable robots

## Secrets

- Web Research does **not** require a Brave / search API key
- AI provider keys remain in OS keyring / `.env` and are never sent to the sidecar or the webview
- Legacy Brave keyring cleanup is optional via `delete_search_connection`

## Logging

Errors returned to the UI are sanitized (`sanitize_error`). Do not log full page bodies or credentials.

## Related

- [SECURITY.md](./SECURITY.md)
- [CRAWLER_COMPLIANCE.md](./CRAWLER_COMPLIANCE.md)
- [CRAWL4AI_INTEGRATION.md](./CRAWL4AI_INTEGRATION.md)
