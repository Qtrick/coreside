-- Migration 025: Add provenance columns to patch scheduler for model/provider tracking.
-- Enables P1.1 provenance attribution on scheduled patches.

ALTER TABLE patch_scheduler_items ADD COLUMN model TEXT;
ALTER TABLE patch_scheduler_items ADD COLUMN provider TEXT;
