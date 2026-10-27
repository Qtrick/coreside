-- Enforce atomic scheduled slot uniqueness on automation_runs.
-- Prevents duplicate execution claims for the same scheduled slot across concurrent workers/processes.
DELETE FROM automation_runs
WHERE scheduled_at IS NOT NULL
  AND id NOT IN (
      SELECT id FROM (
          SELECT id, ROW_NUMBER() OVER (
              PARTITION BY automation_id, scheduled_at ORDER BY started_at DESC
          ) AS rn FROM automation_runs WHERE scheduled_at IS NOT NULL
      ) WHERE rn = 1
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_automation_runs_scheduled_slot
  ON automation_runs (automation_id, scheduled_at)
  WHERE scheduled_at IS NOT NULL;
