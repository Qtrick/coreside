-- Monotonic per-model data revision for missed-event reconciliation.
-- Bumped on every committed create/update/delete of generated_data_records.
CREATE TABLE IF NOT EXISTS generated_data_revisions (
    application_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (application_id, model_id)
);
