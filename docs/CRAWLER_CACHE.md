# Crawler cache

Web Research keeps a managed on-disk cache for Crawl4AI browser/content data under the Coreside crawler data root (not in SQLite, and not in the Media Library).

## Soft quota

Default soft quota: **~1 GB** (`DEFAULT_CACHE_QUOTA_BYTES` in the Python sidecar).

`get_crawler_cache_stats` returns:

| Field | Meaning |
| --- | --- |
| `root` | Cache directory path |
| `sizeBytes` | Current size |
| `quotaBytes` | Soft quota |
| `usageRatio` | size / quota |
| `overQuota` | size exceeds quota |

## Cleanup

- **Settings:** Clear research cache → `cleanup_crawler_cache`
- **Automatic:** LRU eviction toward the quota; also on idle shutdown / app quit paths when wired
- Cleanup **must not** delete Media Library assets (paths containing `media_library` are skipped)

Developer scripts: `npm run crawl4ai:stats`, `npm run crawl4ai:clean`.

## Modes

Coreside maps cache modes (`fresh` / `standard` / `offline`) onto Crawl4AI `CacheMode` internally. Consumers do not need to pick cache modes for normal chat research.
