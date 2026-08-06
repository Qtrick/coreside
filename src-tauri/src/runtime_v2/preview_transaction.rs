//! Progressive preview transaction for live operation frames.
//!
//! Holds validated ops discovered during streaming **before** durable
//! `schedule_and_apply`. Speculative surface paint updates an in-memory
//! definition/state clone only — SQLite is not written until turn-end commit.
//!
//! Adapted from Partial Update `UpdateStreamParser` / `runModel` progressive
//! dispatch (MIT License, Copyright (c) 2026 Phil Holden) — see
//! `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md` and `THIRD_PARTY_NOTICES.md`.

use std::collections::HashMap;

use serde_json::{json, Value};
use uuid::Uuid;

use super::operations::AppOperation;
use super::patch::apply_component_op;
use super::streaming::{NdjsonFrameParser, StreamEvent};
use crate::ai::ToolComponent;

/// Speculative in-memory surface clone for progressive paint.
#[derive(Debug, Clone)]
pub struct PreviewSurfaceModel {
    pub surface_id: String,
    pub tool_id: Option<String>,
    pub application_id: Option<String>,
    pub definition: Value,
    pub state: Value,
    pub base_revision: i64,
    pub preview_revision: i64,
}

/// Paint payload for Channel `PreviewSurface` events.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewPaintEvent {
    pub turn_id: String,
    pub tool_id: Option<String>,
    pub surface_id: String,
    pub application_id: Option<String>,
    pub definition_json: Value,
    pub state_json: Value,
    pub revision: i64,
    pub sequence: u64,
}

/// In-memory preview bag for one agent turn.
#[derive(Debug, Clone)]
pub struct PreviewTransaction {
    pub preview_transaction_id: String,
    pub turn_id: String,
    pub accepted: Vec<AppOperation>,
    pub rejected: Vec<RejectedPreviewOp>,
    pub base_revision: Option<i64>,
    pub interrupted: bool,
    pub committed: bool,
    pub surfaces: HashMap<String, PreviewSurfaceModel>,
    pub paint_sequence: u64,
}

impl Default for PreviewTransaction {
    fn default() -> Self {
        Self::new(String::new(), None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPreviewOp {
    pub operation_id: String,
    pub reason: String,
}

/// Channel-facing preview/reject/fatal event produced while ingesting live chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewOpEvent {
    pub operation_id: String,
    pub status: String,
    pub reason: Option<String>,
    pub paint: Option<PreviewPaintEvent>,
}

impl PreviewTransaction {
    pub fn new(turn_id: impl Into<String>, base_revision: Option<i64>) -> Self {
        Self {
            preview_transaction_id: format!("ptxn-{}", Uuid::new_v4()),
            turn_id: turn_id.into(),
            accepted: Vec::new(),
            rejected: Vec::new(),
            base_revision,
            interrupted: false,
            committed: false,
            surfaces: HashMap::new(),
            paint_sequence: 0,
        }
    }

    /// Seed (or replace) a speculative surface clone. Does not touch SQLite.
    pub fn seed_surface(&mut self, model: PreviewSurfaceModel) {
        if self.base_revision.is_none() {
            self.base_revision = Some(model.base_revision);
        }
        self.surfaces.insert(model.surface_id.clone(), model);
    }

    /// Accept a completed, parser-validated operation for later durable apply.
    pub fn accept(&mut self, op: AppOperation) -> Result<(), String> {
        if self.interrupted {
            return Err("preview transaction interrupted".into());
        }
        if self.committed {
            return Err("preview transaction already committed".into());
        }
        if self.accepted.iter().any(|a| a.id == op.id) {
            return Err(format!("duplicate preview operation: {}", op.id));
        }
        self.accepted.push(op);
        Ok(())
    }

    pub fn reject(&mut self, operation_id: impl Into<String>, reason: impl Into<String>) {
        self.rejected.push(RejectedPreviewOp {
            operation_id: operation_id.into(),
            reason: reason.into(),
        });
    }

    fn unaccept(&mut self, operation_id: &str) {
        self.accepted.retain(|op| op.id != operation_id);
    }

    pub fn mark_interrupted(&mut self) {
        self.interrupted = true;
        // Drop accepted ops so turn-end harvest cannot durable-commit after
        // cancel/fatal (surfaces were speculative only).
        self.accepted.clear();
        self.clear_surfaces();
    }

    /// Clear speculative paint after durable commit success.
    pub fn mark_committed(&mut self) {
        self.committed = true;
        self.clear_surfaces();
    }

    pub fn clear_surfaces(&mut self) {
        self.surfaces.clear();
    }

    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub fn accepted_operations(&self) -> &[AppOperation] {
        &self.accepted
    }

    pub fn is_empty(&self) -> bool {
        self.accepted.is_empty()
    }

    pub fn surface(&self, surface_id: &str) -> Option<&PreviewSurfaceModel> {
        self.surfaces.get(surface_id)
    }

    /// Apply an accepted op to the speculative model. Returns a paint event when
    /// the surface definition/state changed. Does not write SQLite.
    pub fn paint_op(
        &mut self,
        op: &AppOperation,
        mut seed: impl FnMut(&str) -> Option<PreviewSurfaceModel>,
    ) -> Result<Option<PreviewPaintEvent>, String> {
        if self.interrupted {
            return Err("preview transaction interrupted".into());
        }
        if self.committed {
            return Err("preview transaction already committed".into());
        }

        match op.op_type.as_str() {
            "component.update_props"
            | "component.insert"
            | "component.remove"
            | "component.replace"
            | "component.move"
            | "component.update_children"
            | "component.update_visibility"
            | "component.update_actions"
            | "chat.inline_surface_update" => {
                let sid = op
                    .target
                    .surface_id
                    .as_deref()
                    .ok_or_else(|| "surfaceId required".to_string())?;
                self.ensure_seeded(sid, &mut seed)?;
                self.apply_definition_op(sid, op)?;
                Ok(Some(self.make_paint(sid)?))
            }
            "state.set" | "state.patch" => {
                let sid = op
                    .target
                    .surface_id
                    .as_deref()
                    .ok_or_else(|| "surfaceId required".to_string())?;
                self.ensure_seeded(sid, &mut seed)?;
                let state = op
                    .payload
                    .get("state")
                    .cloned()
                    .unwrap_or_else(|| op.payload.clone());
                let model = self
                    .surfaces
                    .get_mut(sid)
                    .ok_or_else(|| format!("preview surface missing: {sid}"))?;
                if op.op_type == "state.patch" {
                    merge_json_objects(&mut model.state, &state);
                } else {
                    model.state = state;
                }
                model.preview_revision = model.preview_revision.saturating_add(1);
                Ok(Some(self.make_paint(sid)?))
            }
            "surface.create" | "tool.full_replace" => {
                let tool = op
                    .payload
                    .get("tool")
                    .cloned()
                    .unwrap_or_else(|| op.payload.clone());
                let tool_id = tool
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| op.target.tool_id.clone())
                    .ok_or_else(|| "tool id required for surface create/replace".to_string())?;
                let surface_id = op
                    .target
                    .surface_id
                    .clone()
                    .unwrap_or_else(|| format!("surf-{tool_id}"));
                let base = self
                    .surfaces
                    .get(&surface_id)
                    .map(|m| m.base_revision)
                    .or(self.base_revision)
                    .unwrap_or(0);
                self.seed_surface(PreviewSurfaceModel {
                    surface_id: surface_id.clone(),
                    tool_id: Some(tool_id.clone()),
                    application_id: Some(tool_id),
                    definition: tool,
                    state: self
                        .surfaces
                        .get(&surface_id)
                        .map(|m| m.state.clone())
                        .unwrap_or_else(|| json!({})),
                    base_revision: base,
                    preview_revision: base.saturating_add(1),
                });
                Ok(Some(self.make_paint(&surface_id)?))
            }
            // No surface paint — still valid preview ops (status / settings / etc.).
            _ => Ok(None),
        }
    }

    fn ensure_seeded(
        &mut self,
        surface_id: &str,
        seed: &mut impl FnMut(&str) -> Option<PreviewSurfaceModel>,
    ) -> Result<(), String> {
        if self.surfaces.contains_key(surface_id) {
            return Ok(());
        }
        let model = seed(surface_id)
            .ok_or_else(|| format!("invalid preview target: surface not found ({surface_id})"))?;
        if model.surface_id != surface_id {
            return Err(format!(
                "invalid preview target: seed surface id mismatch ({})",
                model.surface_id
            ));
        }
        self.seed_surface(model);
        Ok(())
    }

    fn apply_definition_op(&mut self, surface_id: &str, op: &AppOperation) -> Result<(), String> {
        let model = self
            .surfaces
            .get_mut(surface_id)
            .ok_or_else(|| format!("preview surface missing: {surface_id}"))?;

        if op.op_type == "chat.inline_surface_update" {
            if let Some(definition) = op.payload.get("definition") {
                model.definition = definition.clone();
                model.preview_revision = model.preview_revision.saturating_add(1);
                return Ok(());
            }
            return Err("inline surface update missing definition".into());
        }

        let mut components: Vec<ToolComponent> = model
            .definition
            .get("components")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // ponytail: unspecified base_revision means "stack on current speculative".
        apply_component_op(
            &mut components,
            &op.op_type,
            op.target.component_id.as_deref(),
            op.target.parent_id.as_deref(),
            op.base_revision.or(Some(model.preview_revision)),
            model.preview_revision,
            &op.payload,
        )
        .map_err(|e| e.to_string())?;

        if let Some(obj) = model.definition.as_object_mut() {
            obj.insert(
                "components".into(),
                serde_json::to_value(&components).unwrap_or_else(|_| json!([])),
            );
        } else {
            model.definition = json!({ "components": components });
        }
        // Preserve tool identity when definition is component-only.
        if model.definition.get("id").is_none() {
            if let Some(tool_id) = &model.tool_id {
                if let Some(obj) = model.definition.as_object_mut() {
                    obj.insert("id".into(), json!(tool_id));
                }
            }
        }
        model.preview_revision = model.preview_revision.saturating_add(1);
        Ok(())
    }

    fn make_paint(&mut self, surface_id: &str) -> Result<PreviewPaintEvent, String> {
        let model = self
            .surfaces
            .get(surface_id)
            .ok_or_else(|| format!("preview surface missing: {surface_id}"))?;
        self.paint_sequence = self.paint_sequence.saturating_add(1);
        Ok(PreviewPaintEvent {
            turn_id: self.turn_id.clone(),
            tool_id: model.tool_id.clone(),
            surface_id: model.surface_id.clone(),
            application_id: model.application_id.clone(),
            definition_json: model.definition.clone(),
            state_json: model.state.clone(),
            revision: model.preview_revision,
            sequence: self.paint_sequence,
        })
    }
}

fn merge_json_objects(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(dst), Value::Object(src)) => {
            for (k, v) in src {
                match dst.get_mut(k) {
                    Some(existing) if existing.is_object() && v.is_object() => {
                        merge_json_objects(existing, v);
                    }
                    _ => {
                        dst.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}

/// Classify parser errors: incomplete buffering vs fatal framing loss.
fn classify_parser_error(err: &str) -> (&'static str, bool) {
    use super::streaming::StreamParseError;
    let (kind, fatal) = StreamParseError::classify_message(err);
    let status = match kind {
        super::streaming::StreamParseErrorKind::Incomplete => "incomplete",
        super::streaming::StreamParseErrorKind::Malformed
        | super::streaming::StreamParseErrorKind::Unrecognized => "rejected",
        super::streaming::StreamParseErrorKind::LimitExceeded
        | super::streaming::StreamParseErrorKind::Utf8
        | super::streaming::StreamParseErrorKind::Halted => "fatal",
    };
    (status, fatal)
}

/// Ingest provider `coreside.ops.v1` frames into a speculative preview.
/// Speculative paint only — durable apply requires [`ProgressiveOpsParser::durable_operations`].
pub fn ingest_progressive_chunk_with_seed(
    parser: &mut super::progressive_ops::ProgressiveOpsParser,
    preview: &mut PreviewTransaction,
    chunk: &str,
    mut seed: impl FnMut(&str) -> Option<PreviewSurfaceModel>,
) -> Vec<PreviewOpEvent> {
    use super::progressive_ops::ProgressiveOpsFrame;

    let mut out = Vec::new();
    if chunk.is_empty() || preview.interrupted || preview.committed {
        return out;
    }
    for ev in parser.push(chunk) {
        match ev {
            Ok(ProgressiveOpsFrame::Op { op, .. }) => {
                let paint_events = accept_and_paint(preview, op, &mut seed);
                // Response rules: any rejected sibling fails the entire group.
                // Parser already recorded the op — clear it so durable apply cannot proceed.
                if paint_events.iter().any(|e| e.status == "rejected") {
                    let reason = paint_events
                        .iter()
                        .find_map(|e| e.reason.clone())
                        .unwrap_or_else(|| "rejected sibling operation".into());
                    parser.fail_group(format!("rejected sibling operation: {reason}"));
                    preview.mark_interrupted();
                    out.extend(paint_events);
                    out.push(PreviewOpEvent {
                        operation_id: "parser".into(),
                        status: "fatal".into(),
                        reason: Some(reason),
                        paint: None,
                    });
                    return out;
                }
                out.extend(paint_events);
            }
            Ok(ProgressiveOpsFrame::Abort { reason, .. }) => {
                preview.mark_interrupted();
                out.push(PreviewOpEvent {
                    operation_id: String::new(),
                    status: "interrupted".into(),
                    reason: Some(reason.unwrap_or_else(|| "progressive abort".into())),
                    paint: None,
                });
            }
            Ok(ProgressiveOpsFrame::Complete { .. }) => {
                out.push(PreviewOpEvent {
                    operation_id: String::new(),
                    status: "terminal_complete".into(),
                    reason: None,
                    paint: None,
                });
            }
            Ok(ProgressiveOpsFrame::Start { .. }) => {}
            Err(err) => {
                let (status, fatal) = classify_parser_error(&err);
                if status == "incomplete" && !fatal {
                    continue;
                }
                // Any rejected/malformed/halted sibling invalidates the group.
                preview.mark_interrupted();
                preview.reject("parser", err.clone());
                out.push(PreviewOpEvent {
                    operation_id: "parser".into(),
                    status: "fatal".into(),
                    reason: Some(err),
                    paint: None,
                });
            }
        }
    }
    out
}

/// Finish the progressive stream; missing/invalid terminal clears speculative ops.
pub fn finish_progressive_ingest(
    parser: &mut super::progressive_ops::ProgressiveOpsParser,
    preview: &mut PreviewTransaction,
) -> Vec<PreviewOpEvent> {
    let mut out = Vec::new();
    if preview.interrupted || preview.committed {
        let _ = parser.finish();
        return out;
    }
    for ev in parser.finish() {
        if let Err(err) = ev {
            preview.mark_interrupted();
            preview.reject("parser", err.clone());
            out.push(PreviewOpEvent {
                operation_id: "parser".into(),
                status: "fatal".into(),
                reason: Some(err),
                paint: None,
            });
        }
    }
    if parser.durable_operations().is_none() && parser.started() && !preview.interrupted {
        preview.mark_interrupted();
    }
    out
}

/// Push a live TextDelta chunk through the canonical NDJSON parser.
///
/// Yields preview/reject/fatal events for complete frames. When `seed` can
/// supply a surface clone, accepted paint ops also produce `paint` payloads.
/// Incomplete JSON blobs produce nothing until a newline-delimited StreamEvent
/// frame completes — that is intentional.
pub fn ingest_live_chunk(
    parser: &mut NdjsonFrameParser,
    preview: &mut PreviewTransaction,
    chunk: &str,
) -> Vec<PreviewOpEvent> {
    ingest_live_chunk_with_seed(parser, preview, chunk, |_| None)
}

/// Like [`ingest_live_chunk`], but seeds speculative surfaces on demand.
pub fn ingest_live_chunk_with_seed(
    parser: &mut NdjsonFrameParser,
    preview: &mut PreviewTransaction,
    chunk: &str,
    mut seed: impl FnMut(&str) -> Option<PreviewSurfaceModel>,
) -> Vec<PreviewOpEvent> {
    let mut out = Vec::new();
    if chunk.is_empty() || preview.interrupted || preview.committed {
        return out;
    }
    for ev in parser.push(chunk) {
        match ev {
            Ok(StreamEvent::OperationFrameCompleted { operation }) => {
                out.extend(accept_and_paint(preview, operation, &mut seed));
            }
            Ok(StreamEvent::OperationRejected {
                operation_id,
                reason,
            }) => {
                preview.reject(operation_id.clone(), reason.clone());
                out.push(PreviewOpEvent {
                    operation_id,
                    status: "rejected".into(),
                    reason: Some(reason),
                    paint: None,
                });
            }
            Ok(StreamEvent::PreviewUpdated { operations }) => {
                for operation in operations {
                    out.extend(accept_and_paint(preview, operation, &mut seed));
                }
            }
            Ok(StreamEvent::TurnCancelled { .. }) => {
                preview.mark_interrupted();
                out.push(PreviewOpEvent {
                    operation_id: String::new(),
                    status: "interrupted".into(),
                    reason: Some("turn cancelled".into()),
                    paint: None,
                });
            }
            Ok(StreamEvent::TurnFailed { error, .. }) => {
                preview.mark_interrupted();
                out.push(PreviewOpEvent {
                    operation_id: String::new(),
                    status: "fatal".into(),
                    reason: Some(error),
                    paint: None,
                });
            }
            Ok(_) => {}
            Err(err) => {
                let (kind, fatal) = classify_parser_error(&err);
                if kind == "incomplete" && !fatal {
                    // Still buffering a frame / JSON blob — do not surface.
                    continue;
                }
                if fatal {
                    preview.mark_interrupted();
                }
                preview.reject("parser", err.clone());
                out.push(PreviewOpEvent {
                    operation_id: "parser".into(),
                    status: if fatal {
                        "fatal".into()
                    } else {
                        "rejected".into()
                    },
                    reason: Some(err),
                    paint: None,
                });
            }
        }
    }
    out
}

fn accept_and_paint(
    preview: &mut PreviewTransaction,
    operation: AppOperation,
    seed: &mut impl FnMut(&str) -> Option<PreviewSurfaceModel>,
) -> Vec<PreviewOpEvent> {
    let id = operation.id.clone();
    match preview.accept(operation.clone()) {
        Ok(()) => match preview.paint_op(&operation, seed) {
            Ok(paint) => vec![PreviewOpEvent {
                operation_id: id,
                status: "preview".into(),
                reason: None,
                paint,
            }],
            Err(reason) => {
                preview.unaccept(&id);
                preview.reject(id.clone(), reason.clone());
                vec![PreviewOpEvent {
                    operation_id: id,
                    status: "rejected".into(),
                    reason: Some(reason),
                    paint: None,
                }]
            }
        },
        Err(reason) => {
            preview.reject(id.clone(), reason.clone());
            vec![PreviewOpEvent {
                operation_id: id,
                status: "rejected".into(),
                reason: Some(reason),
                paint: None,
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::runtime_v2::surfaces::{get_surface, SurfaceRecord};
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_op(id: &str) -> AppOperation {
        serde_json::from_value(json!({
            "id": id,
            "type": "component.update_props",
            "target": {"surfaceId": "s1", "componentId": "c1"},
            "payload": {"maximum": 10}
        }))
        .expect("sample op")
    }

    fn sample_definition() -> Value {
        json!({
            "id": "tool-1",
            "name": "Counter",
            "description": "test",
            "components": [{
                "id": "c1",
                "type": "progress",
                "props": { "maximum": 5, "value": 1 }
            }]
        })
    }

    fn seed_s1() -> PreviewSurfaceModel {
        PreviewSurfaceModel {
            surface_id: "s1".into(),
            tool_id: Some("tool-1".into()),
            application_id: Some("tool-1".into()),
            definition: sample_definition(),
            state: json!({ "n": 1 }),
            base_revision: 3,
            preview_revision: 3,
        }
    }

    fn op_frame_line(id: &str) -> String {
        json!({
            "type": "operation.frame_completed",
            "operation": {
                "id": id,
                "type": "component.update_props",
                "target": {"surfaceId": "s1", "componentId": "c1"},
                "payload": {"maximum": 10}
            }
        })
        .to_string()
            + "\n"
    }

    #[test]
    fn accept_and_reject_ops() {
        let mut preview = PreviewTransaction::new("turn-1", Some(3));
        assert_eq!(preview.turn_id(), "turn-1");
        assert!(preview.preview_transaction_id.starts_with("ptxn-"));
        assert!(preview.accept(sample_op("op-a")).is_ok());
        assert_eq!(preview.accepted_operations().len(), 1);
        assert!(preview.accept(sample_op("op-a")).is_err());
        preview.reject("op-b", "invalid");
        assert_eq!(preview.rejected.len(), 1);
        assert_eq!(preview.base_revision, Some(3));
    }

    #[test]
    fn mark_interrupted_blocks_accept_and_clears_surfaces() {
        let mut preview = PreviewTransaction::new("turn-2", None);
        preview.seed_surface(seed_s1());
        assert!(!preview.surfaces.is_empty());
        preview.mark_interrupted();
        assert!(preview.interrupted);
        assert!(preview.surfaces.is_empty());
        assert!(preview.accept(sample_op("op-x")).is_err());
    }

    #[test]
    fn ingest_live_chunk_emits_preview_before_finish() {
        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-3", None);
        preview.seed_surface(seed_s1());

        let partial = r#"{"type":"operation.frame_completed","operation":{"id":"op-live""#;
        assert!(ingest_live_chunk(&mut parser, &mut preview, partial).is_empty());
        assert!(preview.is_empty());

        let rest = r#","type":"component.update_props","target":{"surfaceId":"s1","componentId":"c1"},"payload":{"maximum":10}}}"#;
        assert!(ingest_live_chunk(&mut parser, &mut preview, rest).is_empty());

        let events = ingest_live_chunk(&mut parser, &mut preview, "\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].status, "preview");
        assert_eq!(events[0].operation_id, "op-live");
        assert!(events[0].paint.is_some());
        assert_eq!(preview.accepted_operations().len(), 1);
        let painted = preview.surface("s1").expect("surface");
        assert_eq!(
            painted.definition["components"][0]["props"]["maximum"],
            json!(10)
        );
        assert_eq!(painted.preview_revision, 4);
    }

    #[test]
    fn progressive_frames_yield_preview_before_turn_complete_frame() {
        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-4", None);
        preview.seed_surface(seed_s1());

        let op_events = ingest_live_chunk(&mut parser, &mut preview, &op_frame_line("op-1"));
        assert_eq!(op_events.len(), 1);
        assert_eq!(op_events[0].status, "preview");

        let done = r#"{"type":"turn.completed","turn_id":"turn-4"}"#.to_string() + "\n";
        let after = ingest_live_chunk(&mut parser, &mut preview, &done);
        assert!(
            after.is_empty(),
            "turn.completed must not fabricate op previews"
        );
        assert_eq!(preview.accepted_operations().len(), 1);
    }

    #[test]
    fn paint_model_updates_before_commit_without_db_write() {
        use crate::db::{create_conversation, DEFAULT_WORKSPACE_ID};
        use crate::runtime_v2::surfaces::create_inline_surface;

        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("preview.db")).unwrap();
        let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Chat", None).unwrap();
        let surface = create_inline_surface(
            &mut db,
            &conv.id,
            None,
            None,
            "Counter",
            &sample_definition(),
            &[],
        )
        .expect("create surface");
        let before = get_surface(&db, &surface.id).expect("get");
        let before_def = before.definition.clone();
        let before_rev = before.current_revision;

        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-paint", Some(before_rev));
        let sid = surface.id.clone();
        let events = ingest_live_chunk_with_seed(
            &mut parser,
            &mut preview,
            &op_frame_for_surface("op-paint", &sid),
            |want| {
                if want == sid {
                    Some(PreviewSurfaceModel {
                        surface_id: surface.id.clone(),
                        tool_id: surface.tool_id.clone(),
                        application_id: surface.tool_id.clone(),
                        definition: before_def.clone(),
                        state: json!({}),
                        base_revision: before_rev,
                        preview_revision: before_rev,
                    })
                } else {
                    None
                }
            },
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].status, "preview");
        let paint = events[0].paint.as_ref().expect("paint");
        assert_eq!(paint.surface_id, surface.id);
        assert_eq!(
            paint.definition_json["components"][0]["props"]["maximum"],
            json!(10)
        );
        assert!(paint.revision > before_rev);

        // Durable DB unchanged during preview.
        let after: SurfaceRecord = get_surface(&db, &surface.id).expect("get after");
        assert_eq!(after.definition, before_def);
        assert_eq!(after.current_revision, before_rev);

        preview.mark_committed();
        assert!(preview.surfaces.is_empty());
        assert!(preview.committed);
    }

    #[test]
    fn cancel_clears_speculative_model() {
        let mut preview = PreviewTransaction::new("turn-cancel", None);
        preview.seed_surface(seed_s1());
        assert!(preview.accept(sample_op("op-c")).is_ok());
        assert!(preview
            .paint_op(&sample_op("op-c"), |_| None)
            .unwrap()
            .is_some());
        assert!(!preview.surfaces.is_empty());
        assert!(!preview.is_empty());

        let mut parser = NdjsonFrameParser::new();
        let events = ingest_live_chunk(
            &mut parser,
            &mut preview,
            &(r#"{"type":"turn.cancelled","turn_id":"turn-cancel"}"#.to_string() + "\n"),
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].status, "interrupted");
        assert!(preview.interrupted);
        assert!(preview.surfaces.is_empty());
        assert!(
            preview.is_empty(),
            "interrupt must clear accepted ops to block durable harvest"
        );
    }

    #[test]
    fn interrupt_blocks_further_accept() {
        let mut preview = PreviewTransaction::new("turn-int", None);
        preview.mark_interrupted();
        assert!(preview.accept(sample_op("op-x")).is_err());
        assert!(preview.is_empty());
    }

    #[test]
    fn invalid_target_rejected() {
        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-bad", None);
        let events = ingest_live_chunk(&mut parser, &mut preview, &op_frame_line("op-missing"));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].status, "rejected");
        assert!(events[0]
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("surface not found"));
        assert!(preview.accepted.is_empty());
        assert_eq!(preview.rejected.len(), 1);
    }

    #[test]
    fn progressive_rejected_sibling_clears_durable_ops() {
        use super::super::progressive_ops::{ProgressiveOpsExpect, ProgressiveOpsParser};

        let mut parser = ProgressiveOpsParser::new(ProgressiveOpsExpect {
            capability_version: Some("1".into()),
            ..Default::default()
        });
        let mut preview = PreviewTransaction::new("turn-prog-reject", None);
        let start = concat!(
            r#"{"v":"coreside.ops.v1","type":"start","groupId":"g1","schemaVersion":"2","capabilityVersion":"1"}"#,
            "\n"
        );
        let bad_op = concat!(
            r#"{"v":"coreside.ops.v1","type":"op","frameId":1,"groupId":"g1","op":{"id":"op-missing","type":"component.update_props","target":{"surfaceId":"missing","componentId":"c1"},"payload":{"maximum":10}}}"#,
            "\n"
        );
        assert!(
            ingest_progressive_chunk_with_seed(&mut parser, &mut preview, start, |_| None)
                .is_empty()
        );
        let events =
            ingest_progressive_chunk_with_seed(&mut parser, &mut preview, bad_op, |_| None);
        assert!(events.iter().any(|e| e.status == "rejected"));
        assert!(events.iter().any(|e| e.status == "fatal"));
        assert!(preview.interrupted);
        assert!(parser.durable_operations().is_none());
        assert!(parser.speculative_operations().is_empty());
    }

    fn op_frame_for_surface(id: &str, surface_id: &str) -> String {
        json!({
            "type": "operation.frame_completed",
            "operation": {
                "id": id,
                "type": "component.update_props",
                "target": {"surfaceId": surface_id, "componentId": "c1"},
                "payload": {"maximum": 10}
            }
        })
        .to_string()
            + "\n"
    }
}
