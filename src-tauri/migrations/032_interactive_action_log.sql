-- Migration 032: Authoritative interactive application action log
--
-- Every committed rules-engine transition is recorded in order. Rows with
-- action_id '__init' are checkpoints carrying the full baseline snapshot, so a
-- historical state can be reconstructed exactly by re-executing later rows.
-- event_id is unique per surface, making duplicate dispatches idempotent.

CREATE TABLE IF NOT EXISTS interactive_action_log (
    id TEXT PRIMARY KEY NOT NULL,
    surface_id TEXT NOT NULL REFERENCES surfaces(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    event_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    actor TEXT NOT NULL,
    origin TEXT NOT NULL,
    params_json TEXT NOT NULL DEFAULT '{}',
    definition_revision INTEGER NOT NULL,
    pre_state_revision INTEGER NOT NULL,
    post_state_revision INTEGER NOT NULL,
    post_state_hash TEXT NOT NULL,
    snapshot_json TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (surface_id, event_id),
    UNIQUE (surface_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_interactive_action_log_surface
    ON interactive_action_log(surface_id, seq);
