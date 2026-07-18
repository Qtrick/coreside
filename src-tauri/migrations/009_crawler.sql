-- Local Crawl4AI research engine: runtime, jobs, sources, cache metadata.

CREATE TABLE IF NOT EXISTS crawler_runtime (
  id TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL,
  python_path TEXT,
  sidecar_version TEXT,
  crawl4ai_version TEXT,
  last_error TEXT,
  last_started_at TEXT,
  last_stopped_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS crawler_settings (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO crawler_settings (key, value) VALUES
  ('webResearchResourceProfile', 'balanced'),
  ('webResearchEnabled', 'true'),
  ('crawlerCacheQuotaBytes', '1000000000');

CREATE TABLE IF NOT EXISTS crawl_sources (
  id TEXT PRIMARY KEY NOT NULL,
  url TEXT NOT NULL,
  final_url TEXT,
  title TEXT,
  domain TEXT,
  excerpt TEXT,
  content_hash TEXT,
  profile TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_crawled_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_crawl_sources_domain
  ON crawl_sources (domain, last_crawled_at DESC);

CREATE TABLE IF NOT EXISTS crawl_jobs (
  id TEXT PRIMARY KEY NOT NULL,
  request_id TEXT NOT NULL UNIQUE,
  job_type TEXT NOT NULL,
  status TEXT NOT NULL,
  query TEXT,
  seed_url TEXT,
  seed_domain TEXT,
  resource_profile TEXT,
  error_code TEXT,
  error_message TEXT,
  conversation_id TEXT,
  project_id TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  started_at TEXT,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_crawl_jobs_status
  ON crawl_jobs (status, created_at DESC);

CREATE TABLE IF NOT EXISTS crawl_job_sources (
  job_id TEXT NOT NULL REFERENCES crawl_jobs(id) ON DELETE CASCADE,
  source_id TEXT NOT NULL REFERENCES crawl_sources(id) ON DELETE CASCADE,
  rank INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (job_id, source_id)
);

CREATE TABLE IF NOT EXISTS domain_crawl_state (
  domain TEXT PRIMARY KEY NOT NULL,
  last_request_at TEXT,
  success_count INTEGER NOT NULL DEFAULT 0,
  failure_count INTEGER NOT NULL DEFAULT 0,
  robots_status TEXT,
  cooldown_until TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS crawler_cache_metadata (
  id TEXT PRIMARY KEY NOT NULL,
  root_path TEXT NOT NULL,
  size_bytes INTEGER NOT NULL DEFAULT 0,
  quota_bytes INTEGER NOT NULL DEFAULT 1000000000,
  last_cleanup_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
