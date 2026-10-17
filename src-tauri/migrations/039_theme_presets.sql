-- User-saved appearance theme presets (workspace scope; profile DB ownership).
-- Snapshot JSON is validated in Rust; size capped here as a safety backstop.

CREATE TABLE IF NOT EXISTS theme_presets (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  name_normalized TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  schema_version INTEGER NOT NULL DEFAULT 1,
  snapshot_json TEXT NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (length(trim(name)) > 0),
  CHECK (length(name) <= 80),
  CHECK (length(description) <= 280),
  CHECK (length(snapshot_json) > 0),
  CHECK (length(snapshot_json) <= 65536),
  CHECK (schema_version >= 1),
  CHECK (revision >= 1)
);

CREATE UNIQUE INDEX IF NOT EXISTS theme_presets_name_normalized_uq
  ON theme_presets(name_normalized);

CREATE INDEX IF NOT EXISTS theme_presets_updated_at_idx
  ON theme_presets(updated_at DESC);
