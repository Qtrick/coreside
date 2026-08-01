//! Protected Runtime V2 resource limits. The agent cannot modify these.

pub const MAX_OPERATIONS_PER_TURN: usize = 32;
pub const MAX_TRANSACTION_GROUPS_PER_TURN: usize = 8;
pub const MAX_COMPONENT_TREE_DEPTH: usize = 24;
pub const MAX_COMPONENTS_PER_SURFACE: usize = 200;
pub const MAX_SURFACES_PER_CONVERSATION: usize = 64;
pub const MAX_INLINE_SURFACES_VISIBLE: usize = 12;
pub const MAX_SUBSCRIPTIONS_PER_SURFACE: usize = 16;
pub const MAX_EVENT_DEPTH: usize = 8;
pub const MAX_EVENTS_PER_SURFACE_PER_MINUTE: usize = 20;
pub const MAX_IDENTICAL_EVENTS_PER_INTERVAL: usize = 3;
pub const MAX_SVG_NODES: usize = 500;
pub const MAX_CHART_POINTS: usize = 2_000;
pub const MAX_CANVAS_OBJECTS: usize = 200;
pub const MAX_CODE_EDITOR_CHARS: usize = 200_000;
pub const MAX_DEFINITION_JSON_BYTES: usize = 512_000;
pub const MAX_STATE_JSON_BYTES: usize = 256_000;
pub const MAX_QUEUED_TURNS: usize = 5;
pub const MAX_BRANCH_DEPTH: usize = 32;
pub const MAX_SNAPSHOTS_PER_CONVERSATION: usize = 50;
pub const MAX_DIAGNOSTICS_RETENTION: usize = 100;
pub const MAX_REPLAY_OPS_LOADED: usize = 500;
pub const EVENT_COOLDOWN_MS: u64 = 250;
/// How long a surface stays suspended after the loop breaker trips. The breaker
/// stops a runaway loop; it is not a security boundary, so it must release once
/// the surface goes quiet or the tool would stay dead until the app restarts.
pub const EVENT_SUSPENSION_SECS: u64 = 60;
/// Idempotency keys retained in memory. The bus lives for the whole process, so
/// this set is bounded to keep long sessions from growing without limit.
pub const MAX_IDEMPOTENCY_KEYS: usize = 5_000;
pub const MAX_MANIFEST_JSON_BYTES: usize = 256_000;
pub const MAX_GENERATED_RECORDS_PER_QUERY: usize = 500;
pub const MAX_ANIMATED_COMPONENTS: usize = 8;
pub const MAX_REPAIR_ATTEMPTS: usize = 3;
pub const MAX_PATCH_QUEUE: usize = 64;
pub const MAX_PATCH_QUEUE_BYTES: usize = 1_000_000;
pub const MAX_PREVIEW_HZ: u32 = 8;
pub const MAX_CONTEXT_LEDGER_PER_CONVERSATION: usize = 200;

/// Compact JSON for agent capability introspection (agent cannot raise ceilings).
pub fn limits_json() -> serde_json::Value {
    serde_json::json!({
        "maxOperationsPerTurn": MAX_OPERATIONS_PER_TURN,
        "maxComponentsPerSurface": MAX_COMPONENTS_PER_SURFACE,
        "maxComponentTreeDepth": MAX_COMPONENT_TREE_DEPTH,
        "maxChartPoints": MAX_CHART_POINTS,
        "maxSvgNodes": MAX_SVG_NODES,
        "maxManifestBytes": MAX_MANIFEST_JSON_BYTES,
        "maxGeneratedRecordsPerQuery": MAX_GENERATED_RECORDS_PER_QUERY,
        "maxAnimatedComponents": MAX_ANIMATED_COMPONENTS,
        "maxRepairAttempts": MAX_REPAIR_ATTEMPTS,
        "maxPatchQueue": MAX_PATCH_QUEUE,
        "maxPatchQueueBytes": MAX_PATCH_QUEUE_BYTES,
        "maxPreviewHz": MAX_PREVIEW_HZ,
        "maxContextLedgerPerConversation": MAX_CONTEXT_LEDGER_PER_CONVERSATION,
    })
}
