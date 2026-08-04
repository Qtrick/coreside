//! Progressive preview transaction for live operation frames.
//!
//! Holds validated ops discovered during streaming **before** durable
//! `schedule_and_apply`. Preview is Channel-visible only; commit still runs
//! once at turn end.
//!
//! Adapted from Partial Update `UpdateStreamParser` / `runModel` progressive
//! dispatch (MIT License, Copyright (c) 2026 Phil Holden) — see
//! `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md` and `THIRD_PARTY_NOTICES.md`.

use super::operations::AppOperation;
use super::streaming::{NdjsonFrameParser, StreamEvent};

/// In-memory preview bag for one agent turn.
#[derive(Debug, Clone, Default)]
pub struct PreviewTransaction {
    pub turn_id: String,
    pub accepted: Vec<AppOperation>,
    pub rejected: Vec<RejectedPreviewOp>,
    pub base_revision: Option<i64>,
    pub interrupted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPreviewOp {
    pub operation_id: String,
    pub reason: String,
}

/// Channel-facing preview/reject event produced while ingesting live chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewOpEvent {
    pub operation_id: String,
    pub status: String,
}

impl PreviewTransaction {
    pub fn new(turn_id: impl Into<String>, base_revision: Option<i64>) -> Self {
        Self {
            turn_id: turn_id.into(),
            accepted: Vec::new(),
            rejected: Vec::new(),
            base_revision,
            interrupted: false,
        }
    }

    /// Accept a completed, parser-validated operation for later durable apply.
    pub fn accept(&mut self, op: AppOperation) -> Result<(), String> {
        if self.interrupted {
            return Err("preview transaction interrupted".into());
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

    pub fn mark_interrupted(&mut self) {
        self.interrupted = true;
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
}

/// Push a live TextDelta chunk through the canonical NDJSON parser.
///
/// Yields preview/reject events for complete `OperationFrameCompleted` frames.
/// Incomplete JSON blobs (typical OpenAI assistant JSON) produce nothing until
/// a newline-delimited StreamEvent frame completes — that is intentional.
pub fn ingest_live_chunk(
    parser: &mut NdjsonFrameParser,
    preview: &mut PreviewTransaction,
    chunk: &str,
) -> Vec<PreviewOpEvent> {
    let mut out = Vec::new();
    if chunk.is_empty() || preview.interrupted {
        return out;
    }
    for ev in parser.push(chunk) {
        match ev {
            Ok(StreamEvent::OperationFrameCompleted { operation }) => {
                let id = operation.id.clone();
                match preview.accept(operation) {
                    Ok(()) => out.push(PreviewOpEvent {
                        operation_id: id,
                        status: "preview".into(),
                    }),
                    Err(reason) => {
                        preview.reject(id.clone(), reason);
                        out.push(PreviewOpEvent {
                            operation_id: id,
                            status: "rejected".into(),
                        });
                    }
                }
            }
            Ok(StreamEvent::OperationRejected {
                operation_id,
                reason,
            }) => {
                preview.reject(operation_id.clone(), reason);
                out.push(PreviewOpEvent {
                    operation_id,
                    status: "rejected".into(),
                });
            }
            Ok(StreamEvent::PreviewUpdated { operations }) => {
                // Canonical live path does not emit this; keep for completeness.
                for operation in operations {
                    let id = operation.id.clone();
                    match preview.accept(operation) {
                        Ok(()) => out.push(PreviewOpEvent {
                            operation_id: id,
                            status: "preview".into(),
                        }),
                        Err(reason) => {
                            preview.reject(id.clone(), reason);
                            out.push(PreviewOpEvent {
                                operation_id: id,
                                status: "rejected".into(),
                            });
                        }
                    }
                }
            }
            // Framing noise while buffering a single JSON assistant blob is expected.
            Ok(_) | Err(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_op(id: &str) -> AppOperation {
        serde_json::from_value(json!({
            "id": id,
            "type": "component.update_props",
            "target": {"surfaceId": "s1", "componentId": "c1"},
            "payload": {"maximum": 10}
        }))
        .expect("sample op")
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
        assert!(preview.accept(sample_op("op-a")).is_ok());
        assert_eq!(preview.accepted_operations().len(), 1);
        assert!(preview.accept(sample_op("op-a")).is_err());
        preview.reject("op-b", "invalid");
        assert_eq!(preview.rejected.len(), 1);
        assert_eq!(preview.base_revision, Some(3));
    }

    #[test]
    fn mark_interrupted_blocks_accept() {
        let mut preview = PreviewTransaction::new("turn-2", None);
        preview.mark_interrupted();
        assert!(preview.interrupted);
        assert!(preview.accept(sample_op("op-x")).is_err());
    }

    #[test]
    fn ingest_live_chunk_emits_preview_before_finish() {
        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-3", None);

        let partial = r#"{"type":"operation.frame_completed","operation":{"id":"op-live""#;
        assert!(ingest_live_chunk(&mut parser, &mut preview, partial).is_empty());
        assert!(preview.is_empty());

        let rest = r#","type":"component.update_props","target":{"surfaceId":"s1","componentId":"c1"},"payload":{"maximum":10}}}"#;
        // Still incomplete without newline.
        assert!(ingest_live_chunk(&mut parser, &mut preview, rest).is_empty());

        let events = ingest_live_chunk(&mut parser, &mut preview, "\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].status, "preview");
        assert_eq!(events[0].operation_id, "op-live");
        assert_eq!(preview.accepted_operations().len(), 1);
    }

    #[test]
    fn progressive_frames_yield_preview_before_turn_complete_frame() {
        let mut parser = NdjsonFrameParser::new();
        let mut preview = PreviewTransaction::new("turn-4", None);

        let op_events = ingest_live_chunk(&mut parser, &mut preview, &op_frame_line("op-1"));
        assert_eq!(op_events.len(), 1);
        assert_eq!(op_events[0].status, "preview");

        let done = r#"{"type":"turn.completed","turn_id":"turn-4"}"#.to_string() + "\n";
        let after = ingest_live_chunk(&mut parser, &mut preview, &done);
        assert!(after.is_empty(), "turn.completed must not fabricate op previews");
        assert_eq!(preview.accepted_operations().len(), 1);
    }
}
