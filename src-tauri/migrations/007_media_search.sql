-- Media library + optional search session metadata (no credentials).

CREATE TABLE IF NOT EXISTS media_assets (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  category TEXT NOT NULL,
  title TEXT NOT NULL,
  local_filename TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER,
  height INTEGER,
  duration_ms INTEGER,
  content_hash TEXT NOT NULL,
  source_url TEXT,
  source_page_url TEXT,
  creator TEXT,
  license TEXT,
  attribution TEXT,
  validation_status TEXT NOT NULL DEFAULT 'ok',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_used_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_media_assets_hash
  ON media_assets (content_hash);

CREATE INDEX IF NOT EXISTS idx_media_assets_project
  ON media_assets (project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS search_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT,
  project_id TEXT,
  search_type TEXT NOT NULL,
  query TEXT NOT NULL,
  provider TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS search_results (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL REFERENCES search_sessions(id) ON DELETE CASCADE,
  result_type TEXT NOT NULL,
  title TEXT,
  url TEXT,
  display_domain TEXT,
  snippet TEXT,
  thumbnail_url TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_search_results_session
  ON search_results (session_id);

CREATE INDEX IF NOT EXISTS idx_search_sessions_project
  ON search_sessions (project_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_sessions_conversation
  ON search_sessions (conversation_id);
