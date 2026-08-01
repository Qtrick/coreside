-- Registered action runtime: grants, approvals, audit, build failures.
-- Runtime action authority is separate from kernel operation risk.

CREATE TABLE IF NOT EXISTS runtime_action_grants (
    id TEXT PRIMARY KEY NOT NULL,
    subject TEXT NOT NULL DEFAULT 'local-user',
    application_id TEXT,
    action_name TEXT NOT NULL,
    descriptor_hash TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_json TEXT NOT NULL DEFAULT '{}',
    duration TEXT NOT NULL,
    session_id TEXT,
    source TEXT NOT NULL DEFAULT 'user',
    status TEXT NOT NULL DEFAULT 'active',
    granted_at TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at TEXT,
    expires_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_runtime_grants_lookup
    ON runtime_action_grants(status, action_name, application_id);
CREATE INDEX IF NOT EXISTS idx_runtime_grants_application
    ON runtime_action_grants(application_id);

CREATE TABLE IF NOT EXISTS runtime_approvals (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT,
    action_name TEXT NOT NULL,
    action_title TEXT NOT NULL DEFAULT '',
    risk TEXT NOT NULL DEFAULT 'write',
    critical INTEGER NOT NULL DEFAULT 0,
    input_preview TEXT NOT NULL DEFAULT '',
    -- Frozen call input for exactly-once post-approval execution. Never returned
    -- to generated surfaces; only the trusted decide path may read it.
    input_json TEXT NOT NULL DEFAULT '{}',
    call_hash TEXT NOT NULL,
    descriptor_hash TEXT NOT NULL,
    venue TEXT NOT NULL DEFAULT 'chat',
    presence TEXT NOT NULL DEFAULT 'present',
    session_id TEXT,
    run_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    decided_at TEXT,
    consumed_at TEXT,
    explanation TEXT,
    surface_id TEXT,
    component_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_runtime_approvals_status
    ON runtime_approvals(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_runtime_approvals_call
    ON runtime_approvals(call_hash, status);

-- Compare-and-set receipts: `decided:<id>` and `consumed:<id>`.
-- INSERT OR IGNORE guarantees a decision and a consumption happen at most once.
CREATE TABLE IF NOT EXISTS runtime_approval_claims (
    id TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    actor TEXT NOT NULL DEFAULT 'user',
    venue TEXT NOT NULL DEFAULT 'chat',
    presence TEXT NOT NULL DEFAULT 'present',
    application_id TEXT,
    project_id TEXT,
    conversation_id TEXT,
    run_id TEXT,
    action_name TEXT,
    input_preview TEXT,
    outcome TEXT NOT NULL DEFAULT 'ok',
    risk TEXT,
    decision_source TEXT,
    approval_id TEXT,
    grant_id TEXT,
    detail TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_runtime_audit_created
    ON runtime_audit_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_runtime_audit_application
    ON runtime_audit_events(application_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_runtime_audit_run
    ON runtime_audit_events(run_id, created_at DESC);

CREATE TABLE IF NOT EXISTS application_build_failures (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    safe_message TEXT NOT NULL,
    retryable INTEGER NOT NULL DEFAULT 1,
    request_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    cleared_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_application_build_failures_app
    ON application_build_failures(application_id, cleared_at);

ALTER TABLE automations ADD COLUMN application_id TEXT;
ALTER TABLE automations ADD COLUMN waiting_approval INTEGER NOT NULL DEFAULT 0;
ALTER TABLE automations ADD COLUMN permission_ready INTEGER NOT NULL DEFAULT 1;
