-- Application Kernel: manifests, generated data, permissions, tests, jobs, packages

CREATE TABLE IF NOT EXISTS application_manifests (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL UNIQUE,
    instance_id TEXT NOT NULL UNIQUE,
    schema_version TEXT NOT NULL DEFAULT '1',
    current_version INTEGER NOT NULL DEFAULT 1,
    last_known_good_version INTEGER,
    manifest_json TEXT NOT NULL,
    health_state TEXT NOT NULL DEFAULT 'healthy',
    lifecycle_state TEXT NOT NULL DEFAULT 'active',
    disabled INTEGER NOT NULL DEFAULT 0,
    crash_count INTEGER NOT NULL DEFAULT 0,
    project_id TEXT,
    conversation_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS application_manifest_versions (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL REFERENCES application_manifests(application_id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    manifest_json TEXT NOT NULL,
    transaction_id TEXT,
    validation_status TEXT NOT NULL DEFAULT 'pending',
    test_status TEXT NOT NULL DEFAULT 'pending',
    is_known_good INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (application_id, version)
);

CREATE TABLE IF NOT EXISTS generated_data_models (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    definition_json TEXT NOT NULL,
    current_migration_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (application_id, model_id)
);

CREATE TABLE IF NOT EXISTS generated_data_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    from_version INTEGER NOT NULL,
    to_version INTEGER NOT NULL,
    migration_json TEXT NOT NULL,
    impact_summary TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    rollback_json TEXT,
    applied_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS generated_data_records (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    record_version INTEGER NOT NULL DEFAULT 1,
    data_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_generated_data_app_model
    ON generated_data_records(application_id, model_id);

CREATE TABLE IF NOT EXISTS application_permissions (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    permission TEXT NOT NULL,
    scope_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'granted',
    grant_source TEXT NOT NULL DEFAULT 'user',
    granted_at TEXT,
    revoked_at TEXT,
    UNIQUE (application_id, permission)
);

CREATE TABLE IF NOT EXISTS generated_tests (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    test_id TEXT NOT NULL,
    definition_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_result TEXT,
    last_run_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (application_id, test_id)
);

CREATE TABLE IF NOT EXISTS generated_test_runs (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    test_id TEXT NOT NULL,
    application_version INTEGER,
    status TEXT NOT NULL,
    result_json TEXT NOT NULL DEFAULT '{}',
    duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS application_dependencies (
    id TEXT PRIMARY KEY NOT NULL,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_app_deps_source
    ON application_dependencies(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_app_deps_target
    ON application_dependencies(target_type, target_id);

CREATE TABLE IF NOT EXISTS application_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT,
    turn_id TEXT,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    progress REAL NOT NULL DEFAULT 0,
    current_stage TEXT,
    error_category TEXT,
    result_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS application_health_events (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    safe_message TEXT NOT NULL,
    diagnostic_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS application_packages (
    id TEXT PRIMARY KEY NOT NULL,
    application_id TEXT,
    package_version TEXT NOT NULL,
    direction TEXT NOT NULL,
    status TEXT NOT NULL,
    filename TEXT,
    trust_state TEXT NOT NULL DEFAULT 'untrusted',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS operation_provenance (
    id TEXT PRIMARY KEY NOT NULL,
    transaction_id TEXT NOT NULL UNIQUE,
    turn_id TEXT,
    conversation_id TEXT,
    project_id TEXT,
    source_type TEXT NOT NULL DEFAULT 'user',
    provider TEXT,
    model TEXT,
    prompt_version TEXT,
    protocol_version TEXT,
    compiler_version TEXT,
    parent_revision INTEGER,
    result_revision INTEGER,
    approval_status TEXT,
    approval_at TEXT,
    apply_at TEXT,
    validation_status TEXT,
    test_status TEXT,
    recovery_snapshot_id TEXT,
    citations_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS recovery_state (
    id TEXT PRIMARY KEY NOT NULL CHECK (id = 'local'),
    recovery_mode INTEGER NOT NULL DEFAULT 0,
    disable_user_surfaces INTEGER NOT NULL DEFAULT 0,
    disable_custom_layouts INTEGER NOT NULL DEFAULT 0,
    disable_capability_packs INTEGER NOT NULL DEFAULT 0,
    unclean_shutdown INTEGER NOT NULL DEFAULT 0,
    last_failure_json TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO recovery_state (id, recovery_mode) VALUES ('local', 0);

CREATE TABLE IF NOT EXISTS policy_overrides (
    id TEXT PRIMARY KEY NOT NULL,
    policy_key TEXT NOT NULL UNIQUE,
    decision TEXT NOT NULL,
    scope_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
