# Crawler compliance

Coreside Web Research is designed to respect site access restrictions. Settings shows:

> Coreside respects site restrictions and may be unable to access some sources.

## robots.txt

- Crawl configs always set `check_robots_txt=True`.
- The agent cannot disable robots checks.
- Failure to fetch robots for a URL fails **closed** for that URL (skip / deny), not open crawl.

## Rate limits

- Domain-aware delays between requests to the same host.
- Respect Crawl-delay when present; back off on 429 / 503 when detected.

## Disabled practices

Intentionally **not** used:

- Stealth / undetected browser modes
- Proxies for evasion
- Arbitrary LLM extraction configs from the model
- Treating Crawl4AI as an unrestricted scrape-anything console

## User-visible outcomes

Denied or blocked sources produce empty or partial result sets with compliance-aware error codes (for example robots disallowed), not silent substitution with a third-party search API.

Historical Brave Search citations already stored in chat / search history remain readable.
