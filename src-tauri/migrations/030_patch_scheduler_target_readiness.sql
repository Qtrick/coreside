-- Migration 030: Patch scheduler target readiness & deferred patch handling
--
-- Enables bounded deferred states when operations arrive before their semantic
-- target surface is ready, preventing premature failures or unbounded retries.

ALTER TABLE patch_scheduler_items ADD COLUMN defer_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE patch_scheduler_items ADD COLUMN deferred_target TEXT;
ALTER TABLE patch_scheduler_items ADD COLUMN defer_reason TEXT;

CREATE INDEX IF NOT EXISTS idx_patch_scheduler_deferred
  ON patch_scheduler_items(status, deferred_target);
