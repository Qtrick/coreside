-- Migration 014: Final Partial Update gap completion
-- Preservation, drafts, patch scheduler, routes, context ledger, provider conformance.

CREATE TABLE IF NOT EXISTS component_preservation (
    id TEXT PRIMARY KEY NOT NULL,
    surface_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    preservation_key TEXT,
    policy TEXT NOT NULL DEFAULT 'preserve_if_compatible',
    state_schema_version TEXT NOT NULL DEFAULT '1',
    capability_pack TEXT,
    component_type TEXT NOT NULL DEFAULT '',
    last_preserved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(surface_id, component_id)
);

CREATE INDEX IF NOT EXISTS idx_component_preservation_surface
    ON component_preservation(surface_id);

CREATE TABLE IF NOT EXISTS surface_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    surface_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    form_id TEXT,
    window_id TEXT,
    base_revision INTEGER NOT NULL DEFAULT 0,
    draft_json TEXT NOT NULL DEFAULT '{}',
    persistence_policy TEXT NOT NULL DEFAULT 'session',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(surface_id, component_id, window_id)
);

CREATE INDEX IF NOT EXISTS idx_surface_drafts_surface
    ON surface_drafts(surface_id);

CREATE TABLE IF NOT EXISTS patch_scheduler_items (
    id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL,
    transaction_id TEXT,
    turn_id TEXT,
    conversation_id TEXT,
    surface_id TEXT,
    priority TEXT NOT NULL DEFAULT 'approved_persistent_change',
    status TEXT NOT NULL DEFAULT 'queued',
    sequence_number INTEGER NOT NULL DEFAULT 0,
    depends_on_json TEXT NOT NULL DEFAULT '[]',
    superseded_by TEXT,
    supersedes TEXT,
    supersession_reason TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    error_category TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    applied_at TEXT,
    failed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_patch_scheduler_status
    ON patch_scheduler_items(status, priority, sequence_number);

CREATE TABLE IF NOT EXISTS application_route_state (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    window_id TEXT NOT NULL DEFAULT 'main',
    current_route_id TEXT,
    route_params_json TEXT NOT NULL DEFAULT '{}',
    history_json TEXT NOT NULL DEFAULT '[]',
    history_index INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(application_id, window_id)
);

CREATE TABLE IF NOT EXISTS context_ledger_entries (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    project_id TEXT,
    branch_id TEXT,
    entry_type TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'model_context_only',
    payload_json TEXT NOT NULL DEFAULT '{}',
    summary TEXT NOT NULL DEFAULT '',
    expiration_class TEXT NOT NULL DEFAULT 'session',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_context_ledger_conversation
    ON context_ledger_entries(conversation_id, created_at);

CREATE INDEX IF NOT EXISTS idx_context_ledger_project
    ON context_ledger_entries(project_id, created_at);

CREATE TABLE IF NOT EXISTS provider_conformance (
    id TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL DEFAULT '*',
    profile TEXT NOT NULL DEFAULT 'buffered_structured_response',
    capabilities_json TEXT NOT NULL DEFAULT '{}',
    last_tested_at TEXT,
    benchmark_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(provider_id, model_id)
);

CREATE TABLE IF NOT EXISTS surface_continuity (
    id TEXT PRIMARY KEY NOT NULL,
    surface_id TEXT NOT NULL,
    window_id TEXT NOT NULL DEFAULT 'main',
    focus_json TEXT NOT NULL DEFAULT '{}',
    scroll_json TEXT NOT NULL DEFAULT '{}',
    media_json TEXT NOT NULL DEFAULT '{}',
    suspension_state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(surface_id, window_id)
);

CREATE TABLE IF NOT EXISTS manual_edit_provenance (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT,
    surface_id TEXT,
    source_type TEXT NOT NULL,
    user_action_type TEXT NOT NULL DEFAULT '',
    affected_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_manual_edit_surface
    ON manual_edit_provenance(surface_id, created_at);
