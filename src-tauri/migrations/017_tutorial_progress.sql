-- Tutorial / onboarding progress (RC3.4)

CREATE TABLE IF NOT EXISTS tutorial_progress (
  tutorial_id TEXT NOT NULL,
  tutorial_version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('not_started','in_progress','completed','skipped','superseded')),
  current_step_id TEXT,
  completed_step_ids TEXT NOT NULL DEFAULT '[]',
  started_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  completed_at TEXT,
  skipped_at TEXT,
  last_opened_at TEXT,
  PRIMARY KEY (tutorial_id)
);
