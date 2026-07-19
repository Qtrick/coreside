-- Generative Interface Runtime V2: surfaces, transactions, events, branches, snapshots, queue

CREATE TABLE IF NOT EXISTS surfaces (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL UNIQUE,
    surface_type TEXT NOT NULL DEFAULT 'tool',
    placement TEXT NOT NULL DEFAULT 'tool_canvas',
    owner_type TEXT NOT NULL DEFAULT 'workspace',
    owner_id TEXT,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    project_id TEXT,
    tool_id TEXT REFERENCES tools(id) ON DELETE SET NULL,
    message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    name TEXT NOT NULL DEFAULT '',
    definition_json TEXT NOT NULL,
    current_revision INTEGER NOT NULL DEFAULT 1,
    lifecycle_state TEXT NOT NULL DEFAULT 'active',
    archived INTEGER NOT NULL DEFAULT 0,
    capability_packs_json TEXT NOT NULL DEFAULT '[]',
    error_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS surface_versions (
    id TEXT PRIMARY KEY NOT NULL,
    surface_id TEXT NOT NULL REFERENCES surfaces(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    definition_json TEXT NOT NULL,
    state_schema_json TEXT,
    change_summary TEXT,
    transaction_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (surface_id, revision)
);

CREATE TABLE IF NOT EXISTS surface_state (
    surface_id TEXT PRIMARY KEY NOT NULL REFERENCES surfaces(id) ON DELETE CASCADE,
    state_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS app_transactions (
    id TEXT PRIMARY KEY NOT NULL,
    turn_id TEXT,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    project_id TEXT,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    operations_json TEXT NOT NULL DEFAULT '[]',
    previous_snapshot_json TEXT,
    result_snapshot_json TEXT,
    silent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    applied_at TEXT,
    reverted_at TEXT
);

CREATE TABLE IF NOT EXISTS app_operations (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL REFERENCES app_transactions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    op_type TEXT NOT NULL,
    target_json TEXT NOT NULL DEFAULT '{}',
    base_revision INTEGER,
    payload_json TEXT NOT NULL DEFAULT '{}',
    validation_status TEXT NOT NULL DEFAULT 'pending',
    apply_status TEXT NOT NULL DEFAULT 'pending',
    error_category TEXT,
    idempotency_key TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (transaction_id, sequence)
);

CREATE TABLE IF NOT EXISTS surface_events (
    id TEXT PRIMARY KEY NOT NULL,
    source_json TEXT NOT NULL DEFAULT '{}',
    target_json TEXT NOT NULL DEFAULT '{}',
    scope TEXT NOT NULL DEFAULT 'surface',
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    idempotency_key TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed_at TEXT
);

CREATE TABLE IF NOT EXISTS surface_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    owner_surface_id TEXT REFERENCES surfaces(id) ON DELETE CASCADE,
    source_filter_json TEXT NOT NULL DEFAULT '{}',
    target_json TEXT NOT NULL DEFAULT '{}',
    event_types_json TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS chat_branches (
    id TEXT PRIMARY KEY NOT NULL,
    source_conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    source_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    new_conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_name TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS conversation_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    project_id TEXT,
    description TEXT NOT NULL DEFAULT '',
    read_only_payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS replay_metadata (
    conversation_id TEXT PRIMARY KEY NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    transaction_sequence_json TEXT NOT NULL DEFAULT '[]',
    display_metadata_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS agent_request_queue (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    priority INTEGER NOT NULL DEFAULT 100,
    status TEXT NOT NULL DEFAULT 'queued',
    prompt_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    finished_at TEXT,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS workspace_layouts (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 1,
    layout_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (workspace_id)
);

CREATE TABLE IF NOT EXISTS developer_diagnostics (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    turn_id TEXT,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_surfaces_conversation ON surfaces(conversation_id);
CREATE INDEX IF NOT EXISTS idx_surfaces_tool ON surfaces(tool_id);
CREATE INDEX IF NOT EXISTS idx_surfaces_placement ON surfaces(placement);
CREATE INDEX IF NOT EXISTS idx_surface_versions_surface ON surface_versions(surface_id);
CREATE INDEX IF NOT EXISTS idx_app_transactions_conversation ON app_transactions(conversation_id);
CREATE INDEX IF NOT EXISTS idx_app_operations_transaction ON app_operations(transaction_id);
CREATE INDEX IF NOT EXISTS idx_surface_events_status ON surface_events(status);
CREATE INDEX IF NOT EXISTS idx_chat_branches_source ON chat_branches(source_conversation_id);
CREATE INDEX IF NOT EXISTS idx_agent_queue_conversation ON agent_request_queue(conversation_id, status);
CREATE INDEX IF NOT EXISTS idx_diagnostics_conversation ON developer_diagnostics(conversation_id);

-- Lazy migration: wrap existing tools as tool_canvas surfaces (idempotent)
INSERT OR IGNORE INTO surfaces (
    id, instance_id, surface_type, placement, owner_type, owner_id,
    tool_id, name, definition_json, current_revision, lifecycle_state, archived, created_at, updated_at
)
SELECT
    'surf-' || id,
    'inst-' || id,
    'tool',
    'tool_canvas',
    'workspace',
    workspace_id,
    id,
    name,
    definition_json,
    current_version,
    'active',
    0,
    created_at,
    updated_at
FROM tools;

INSERT OR IGNORE INTO surface_state (surface_id, state_json, updated_at)
SELECT 'surf-' || ts.tool_id, ts.state_json, ts.updated_at
FROM tool_state ts
WHERE EXISTS (SELECT 1 FROM surfaces s WHERE s.id = 'surf-' || ts.tool_id);
