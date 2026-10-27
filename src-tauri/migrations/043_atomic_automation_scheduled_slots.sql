-- Enforce atomic scheduled slot uniqueness using a dedicated claims table.
-- Prevents duplicate execution claims for the same scheduled slot across concurrent workers/processes
-- without destroying historical audit/run records in automation_runs.
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

-- Lossless backfill: record canonical claimed slot for each historical scheduled slot without deleting any runs.
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

