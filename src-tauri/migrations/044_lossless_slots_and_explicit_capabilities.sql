-- Migration 044: Lossless Slots Forward Compatibility & Explicit Capability Authorization
-- 1. Drops legacy unique index on automation_runs if present from development builds.
-- 2. Ensures the dedicated automation_scheduled_slots table exists and is populated.
-- 3. Adds required_capabilities_json to kernel_change_proposals for explicit capability authorization.

DROP INDEX IF EXISTS idx_automation_runs_scheduled_slot;

CREATE TABLE IF NOT EXISTS automation_scheduled_slots (
  automation_id TEXT NOT NULL,
  scheduled_at TEXT NOT NULL,
  run_id TEXT NOT NULL,
  claimed_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (automation_id, scheduled_at),
  FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_automation_scheduled_slots_run
  ON automation_scheduled_slots (run_id);

INSERT OR IGNORE INTO automation_scheduled_slots (automation_id, scheduled_at, run_id, claimed_at)
SELECT automation_id, scheduled_at, id, started_at
FROM (
    SELECT id, automation_id, scheduled_at, started_at,
           ROW_NUMBER() OVER (
               PARTITION BY automation_id, scheduled_at ORDER BY started_at DESC
           ) AS rn
    FROM automation_runs
    WHERE scheduled_at IS NOT NULL
) WHERE rn = 1;

ALTER TABLE kernel_change_proposals ADD COLUMN required_capabilities_json TEXT NOT NULL DEFAULT '[]';
