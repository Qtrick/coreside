# Cloud Crawler Worker Architecture (extension point)

**Product:** Coreside  
**Status:** Documented only — **not implemented**

## Intent

Future separately hosted worker for cloud page extraction when local Crawl4AI is unavailable.

## Requirements for a real worker

- Container with Python + Playwright/Chromium
- Job queue + cancellation
- SSRF / private-network blocks
- Signed callbacks to Supabase
- Usage metering via Edge Function / DB
- No embedding of provider secrets in job payloads beyond signed job tokens

## Non-goals this phase

- Fake “Cloud Crawl” UI
- Running Crawl4AI inside Supabase Edge Functions
