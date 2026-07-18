-- Projects, chat membership, summaries, and local FTS index.

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  icon_key TEXT,
  instructions TEXT,
  summary TEXT,
  summary_updated_at TEXT,
  archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
  pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  wallpaper_json TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_opened_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_projects_archived_updated
  ON projects (archived, updated_at DESC);

-- One project per chat (nullable = unassigned). Keep chats on project delete.
ALTER TABLE conversations ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
ALTER TABLE conversations ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_conversations_project
  ON conversations (project_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS chat_summaries (
  conversation_id TEXT PRIMARY KEY NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  summary TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE VIRTUAL TABLE IF NOT EXISTS message_fts USING fts5(
  message_id UNINDEXED,
  conversation_id UNINDEXED,
  project_id UNINDEXED,
  role UNINDEXED,
  content,
  title,
  tokenize = 'porter unicode61'
);

-- Seed Safe Search preference (Base Setting; agent cannot change via allowlist).
INSERT OR IGNORE INTO settings (key, value) VALUES ('safeSearch', 'standard');
INSERT OR IGNORE INTO settings (key, value) VALUES ('includeProjectContext', 'true');
