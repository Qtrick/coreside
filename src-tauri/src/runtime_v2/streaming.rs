//! Progressive validated operation streaming (NDJSON frames).
//!
//! Incomplete JSON is never applied. Frames are buffered until a complete
//! line/object is available, then validated before preview/apply events.
//!
//! Production path (`push` / `finish`) accepts only canonical `StreamEvent`
//! envelopes. Bare `AppOperation` / `AgentResponseV2` are limited to the
//! explicitly named legacy/compat harvest path.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::limits::{MAX_DEFINITION_JSON_BYTES, MAX_OPERATIONS_PER_TURN, MAX_PATCH_QUEUE_BYTES};
use super::operations::{AgentResponseV2, AppOperation};

/// Max bytes for one NDJSON frame (aligned with surface definition ceiling).
pub const MAX_FRAME_BYTES: usize = MAX_DEFINITION_JSON_BYTES;
/// Max buffered incomplete frame (one frame in flight).
pub const MAX_INCOMPLETE_BUFFER: usize = MAX_FRAME_BYTES;
/// Max total bytes accepted across one parser lifetime (aligned with patch queue).
pub const MAX_TOTAL_STREAM_BYTES: usize = MAX_PATCH_QUEUE_BYTES;
/// Max NDJSON frames/events processed by one parser.
pub const MAX_EVENTS: usize = 512;
/// Max harvested operations (aligned with per-turn operation ceiling).
pub const MAX_OPERATIONS: usize = MAX_OPERATIONS_PER_TURN;

/// Typed NDJSON parser failure — surfaced to preview_transaction for classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamParseErrorKind {
    Incomplete,
    Malformed,
    Unrecognized,
    LimitExceeded,
    Utf8,
    Halted,
}

impl StreamParseErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Malformed => "malformed",
            Self::Unrecognized => "unrecognized",
            Self::LimitExceeded => "limit_exceeded",
            Self::Utf8 => "utf8",
            Self::Halted => "halted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamParseError {
    pub kind: StreamParseErrorKind,
    pub detail: String,
}

impl StreamParseError {
    pub fn new(kind: StreamParseErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn to_line(self) -> String {
        format!("[{}] {}", self.kind.as_str(), self.detail)
    }

    /// Classify legacy string errors (and typed lines) for preview routing.
    pub fn classify_message(raw: &str) -> (StreamParseErrorKind, bool) {
        if let Some(rest) = raw.strip_prefix('[') {
            if let Some((tag, detail)) = rest.split_once("] ") {
                let fatal = match tag {
                    "incomplete" => false,
                    "malformed" | "unrecognized" => false,
                    "limit_exceeded" | "utf8" | "halted" => true,
                    _ => true,
                };
                let kind = match tag {
                    "incomplete" => StreamParseErrorKind::Incomplete,
                    "malformed" => StreamParseErrorKind::Malformed,
                    "unrecognized" => StreamParseErrorKind::Unrecognized,
                    "limit_exceeded" => StreamParseErrorKind::LimitExceeded,
                    "utf8" => StreamParseErrorKind::Utf8,
                    "halted" => StreamParseErrorKind::Halted,
                    _ => StreamParseErrorKind::Malformed,
                };
                let _ = detail;
                return (kind, fatal);
            }
        }
        let lower = raw.to_ascii_lowercase();
        if lower.contains("incomplete frame exceeds")
            || lower.contains("exceeded max total")
            || lower.contains("exceeded max events")
            || lower.contains("exceeded max operations")
            || lower.contains("frame exceeds max")
            || lower.contains("halted")
        {
            return (StreamParseErrorKind::LimitExceeded, true);
        }
        if lower.contains("incomplete frame") || lower.contains("incomplete or invalid") {
            return (StreamParseErrorKind::Incomplete, false);
        }
        if lower.contains("invalid") || lower.contains("unknown") || lower.contains("rejected") {
            return (StreamParseErrorKind::Unrecognized, false);
        }
        (StreamParseErrorKind::Malformed, true)
    }
}

impl std::fmt::Display for StreamParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind.as_str(), self.detail)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    #[serde(rename = "turn.started")]
    TurnStarted { turn_id: String },
    #[serde(rename = "assistant.delta")]
    AssistantDelta { text: String },
    #[serde(rename = "assistant.completed")]
    AssistantCompleted { text: String },
    #[serde(rename = "operation.frame_started")]
    OperationFrameStarted { operation_id: String },
    #[serde(rename = "operation.frame_completed")]
    OperationFrameCompleted { operation: AppOperation },
    #[serde(rename = "operation.validating")]
    OperationValidating { operation_id: String },
    #[serde(rename = "operation.validated")]
    OperationValidated { operation_id: String },
    #[serde(rename = "operation.rejected")]
    OperationRejected {
        operation_id: String,
        reason: String,
    },
    #[serde(rename = "preview.updated")]
    PreviewUpdated { operations: Vec<AppOperation> },
    #[serde(rename = "transaction.ready")]
    TransactionReady { transaction_id: String },
    #[serde(rename = "transaction.applied")]
    TransactionApplied { transaction_id: String },
    #[serde(rename = "turn.completed")]
    TurnCompleted { turn_id: String },
    #[serde(rename = "turn.cancelled")]
    TurnCancelled { turn_id: String },
    #[serde(rename = "turn.failed")]
    TurnFailed { turn_id: String, error: String },
}

#[derive(Clone, Copy)]
enum ParseMode {
    Canonical,
    Legacy,
}

pub(crate) fn stream_err(kind: StreamParseErrorKind, detail: impl Into<String>) -> String {
    StreamParseError::new(kind, detail).to_line()
}

#[derive(Debug, Default)]
pub struct NdjsonFrameParser {
    /// Byte buffer; consumed prefix tracked by `start` (amortized-linear drain).
    buffer: Vec<u8>,
    start: usize,
    total_bytes: usize,
    event_count: usize,
    /// Once set, further push/finish calls return this error (framing lost).
    halted: Option<String>,
    seen_operation_ids: std::collections::HashSet<String>,
    completed_operations: Vec<AppOperation>,
}

impl NdjsonFrameParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Production path: push raw stream text. Accepts only canonical `StreamEvent` frames.
    pub fn push(&mut self, chunk: &str) -> Vec<Result<StreamEvent, String>> {
        self.push_bytes(chunk.as_bytes(), ParseMode::Canonical)
    }

    /// Compatibility path for post-hoc buffered harvest (bare AppOperation /
    /// AgentResponseV2 still accepted). Not used by live production streaming.
    pub fn push_legacy_compat(&mut self, chunk: &str) -> Vec<Result<StreamEvent, String>> {
        self.push_bytes(chunk.as_bytes(), ParseMode::Legacy)
    }

    /// Canonical finish: any non-whitespace leftover is an incomplete-frame error.
    pub fn finish(&mut self) -> Vec<Result<StreamEvent, String>> {
        self.finish_inner(ParseMode::Canonical)
    }

    /// Legacy finish: non-newline remainder may be a complete buffered JSON object
    /// (post-hoc harvest of provider raw_text without a trailing newline).
    pub fn finish_legacy_compat(&mut self) -> Vec<Result<StreamEvent, String>> {
        self.finish_inner(ParseMode::Legacy)
    }

    fn push_bytes(&mut self, chunk: &[u8], mode: ParseMode) -> Vec<Result<StreamEvent, String>> {
        let mut out = Vec::new();
        if let Some(msg) = &self.halted {
            out.push(Err(msg.clone()));
            return out;
        }
        if chunk.is_empty() {
            return out;
        }
        let next_total = self.total_bytes.saturating_add(chunk.len());
        if next_total > MAX_TOTAL_STREAM_BYTES {
            let msg = stream_err(
                StreamParseErrorKind::LimitExceeded,
                format!("stream exceeded max total bytes ({MAX_TOTAL_STREAM_BYTES})"),
            );
            self.halted = Some(msg.clone());
            out.push(Err(msg));
            return out;
        }
        self.total_bytes = next_total;
        self.buffer.extend_from_slice(chunk);

        while let Some(rel) = self.buffer[self.start..].iter().position(|&b| b == b'\n') {
            let line_end = self.start + rel;
            let frame_len = line_end - self.start;
            if frame_len > MAX_FRAME_BYTES {
                self.start = line_end + 1;
                out.push(Err(stream_err(
                    StreamParseErrorKind::LimitExceeded,
                    format!("frame exceeds max size ({MAX_FRAME_BYTES} bytes)"),
                )));
                continue;
            }
            let line = match std::str::from_utf8(&self.buffer[self.start..line_end]) {
                Ok(s) => s.trim().to_string(),
                Err(_) => {
                    self.start = line_end + 1;
                    out.push(Err(stream_err(
                        StreamParseErrorKind::Utf8,
                        "frame is not valid UTF-8",
                    )));
                    continue;
                }
            };
            self.start = line_end + 1;
            if line.is_empty() {
                continue;
            }
            if self.event_count >= MAX_EVENTS {
                let msg = stream_err(
                    StreamParseErrorKind::LimitExceeded,
                    format!("stream event limit exceeded (max {MAX_EVENTS})"),
                );
                self.halted = Some(msg.clone());
                out.push(Err(msg));
                return out;
            }
            self.event_count += 1;
            out.push(match mode {
                ParseMode::Canonical => self.parse_line_canonical(&line),
                ParseMode::Legacy => self.parse_line_legacy(&line),
            });
        }

        let incomplete = self.buffer.len() - self.start;
        if incomplete > MAX_INCOMPLETE_BUFFER {
            let msg = stream_err(
                StreamParseErrorKind::LimitExceeded,
                format!("incomplete frame exceeds max buffer ({MAX_INCOMPLETE_BUFFER} bytes)"),
            );
            // Framing lost — halt so later chunks cannot resync mid-garbage.
            self.halted = Some(msg.clone());
            self.buffer.clear();
            self.start = 0;
            out.push(Err(msg));
            return out;
        }

        // Amortized compact: drain consumed prefix without copying on every line.
        self.compact_if_needed();
        out
    }

    fn finish_inner(&mut self, mode: ParseMode) -> Vec<Result<StreamEvent, String>> {
        if let Some(msg) = &self.halted {
            return vec![Err(msg.clone())];
        }
        let trimmed = trim_ascii_ws(&self.buffer[self.start..]).to_vec();
        self.buffer.clear();
        self.start = 0;
        if trimmed.is_empty() {
            return vec![];
        }
        match mode {
            ParseMode::Canonical => {
                let preview = String::from_utf8_lossy(&trimmed);
                vec![Err(stream_err(
                    StreamParseErrorKind::Incomplete,
                    format!(
                        "incomplete frame at end of stream: {}",
                        truncate(&preview, 80)
                    ),
                ))]
            }
            ParseMode::Legacy => {
                if trimmed.len() > MAX_FRAME_BYTES {
                    return vec![Err(stream_err(
                        StreamParseErrorKind::LimitExceeded,
                        format!("frame exceeds max size ({MAX_FRAME_BYTES} bytes)"),
                    ))];
                }
                let line = match std::str::from_utf8(&trimmed) {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => {
                        return vec![Err(stream_err(
                            StreamParseErrorKind::Utf8,
                            "frame is not valid UTF-8",
                        ))];
                    }
                };
                if line.is_empty() {
                    return vec![];
                }
                if self.event_count >= MAX_EVENTS {
                    let msg = stream_err(
                        StreamParseErrorKind::LimitExceeded,
                        format!("stream event limit exceeded (max {MAX_EVENTS})"),
                    );
                    self.halted = Some(msg.clone());
                    return vec![Err(msg)];
                }
                self.event_count += 1;
                vec![self.parse_line_legacy(&line)]
            }
        }
    }

    fn compact_if_needed(&mut self) {
        // Drain when half the capacity is consumed prefix — amortized O(n) overall.
        if self.start > 0 && self.start >= self.buffer.len() / 2 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
    }

    fn parse_line_canonical(&mut self, line: &str) -> Result<StreamEvent, String> {
        if !line.starts_with('{') || !line.ends_with('}') {
            return Err(stream_err(
                StreamParseErrorKind::Incomplete,
                format!("incomplete or invalid frame: {}", truncate(line, 80)),
            ));
        }
        let value: Value = serde_json::from_str(line).map_err(|e| {
            stream_err(
                StreamParseErrorKind::Malformed,
                format!("malformed frame: {e}"),
            )
        })?;
        let ev: StreamEvent = serde_json::from_value(value).map_err(|_| {
            stream_err(
                StreamParseErrorKind::Unrecognized,
                "unrecognized stream frame (expected StreamEvent)",
            )
        })?;
        self.record_stream_event(&ev)?;
        Ok(ev)
    }

    /// Legacy/compat: StreamEvent, bare AppOperation, or AgentResponseV2.
    fn parse_line_legacy(&mut self, line: &str) -> Result<StreamEvent, String> {
        if !line.starts_with('{') || !line.ends_with('}') {
            return Err(stream_err(
                StreamParseErrorKind::Incomplete,
                format!("incomplete or invalid frame: {}", truncate(line, 80)),
            ));
        }
        let value: Value = serde_json::from_str(line).map_err(|e| {
            stream_err(
                StreamParseErrorKind::Malformed,
                format!("malformed frame: {e}"),
            )
        })?;
        if let Ok(ev) = serde_json::from_value::<StreamEvent>(value.clone()) {
            self.record_stream_event(&ev)?;
            return Ok(ev);
        }
        if let Ok(op) = serde_json::from_value::<AppOperation>(value.clone()) {
            self.record_operation(op.clone())?;
            return Ok(StreamEvent::OperationFrameCompleted { operation: op });
        }
        if let Ok(resp) = serde_json::from_value::<AgentResponseV2>(value) {
            if resp.operations.len()
                > MAX_OPERATIONS.saturating_sub(self.completed_operations.len())
            {
                return Err(stream_err(
                    StreamParseErrorKind::LimitExceeded,
                    format!("operations exceed max of {MAX_OPERATIONS}"),
                ));
            }
            // Validate the whole batch before mutating seen/completed sets.
            let mut batch_ids = std::collections::HashSet::with_capacity(resp.operations.len());
            for op in &resp.operations {
                if !batch_ids.insert(op.id.clone()) || self.seen_operation_ids.contains(&op.id) {
                    return Err(stream_err(
                        StreamParseErrorKind::Unrecognized,
                        format!("duplicate operation frame: {}", op.id),
                    ));
                }
            }
            for op in &resp.operations {
                self.seen_operation_ids.insert(op.id.clone());
            }
            self.completed_operations
                .extend(resp.operations.iter().cloned());
            return Ok(StreamEvent::PreviewUpdated {
                operations: resp.operations,
            });
        }
        Err(stream_err(
            StreamParseErrorKind::Unrecognized,
            "unrecognized stream frame",
        ))
    }

    fn record_stream_event(&mut self, ev: &StreamEvent) -> Result<(), String> {
        if let StreamEvent::OperationFrameCompleted { operation } = ev {
            self.record_operation(operation.clone())?;
        }
        Ok(())
    }

    fn record_operation(&mut self, op: AppOperation) -> Result<(), String> {
        if self.completed_operations.len() >= MAX_OPERATIONS {
            return Err(stream_err(
                StreamParseErrorKind::LimitExceeded,
                format!("operations exceed max of {MAX_OPERATIONS}"),
            ));
        }
        if !self.seen_operation_ids.insert(op.id.clone()) {
            return Err(stream_err(
                StreamParseErrorKind::Unrecognized,
                format!("duplicate operation frame: {}", op.id),
            ));
        }
        self.completed_operations.push(op);
        Ok(())
    }

    pub fn completed_operations(&self) -> &[AppOperation] {
        &self.completed_operations
    }
}

fn trim_ascii_ws(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    // Never slice mid–code-point: `&s[..n]` panics on multibyte UTF-8.
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= n)
        .last()
        .unwrap_or(0);
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn op_frame(id: &str) -> String {
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
    }

    #[test]
    fn buffers_incomplete_then_applies_complete() {
        let mut p = NdjsonFrameParser::new();
        let partial = r#"{"type":"assistant.delta","text":"Hel"#;
        assert!(p.push(partial).is_empty());
        let rest = "lo\"}\n";
        let events = p.push(rest);
        assert_eq!(events.len(), 1);
        assert!(events[0].is_ok());
    }

    #[test]
    fn frame_split_across_chunks() {
        let mut p = NdjsonFrameParser::new();
        let frame = r#"{"type":"turn.started","turn_id":"t1"}"#;
        let mid = frame.len() / 2;
        assert!(p.push(&frame[..mid]).is_empty());
        let events = p.push(&format!("{}\n", &frame[mid..]));
        assert_eq!(events.len(), 1);
        assert!(events[0].is_ok());
    }

    #[test]
    fn multiple_frames_one_chunk() {
        let mut p = NdjsonFrameParser::new();
        let chunk = format!(
            "{}\n{}\n",
            r#"{"type":"turn.started","turn_id":"t1"}"#,
            r#"{"type":"assistant.delta","text":"hi"}"#
        );
        let events = p.push(&chunk);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.is_ok()));
    }

    #[test]
    fn rejects_oversized_frame() {
        let mut p = NdjsonFrameParser::new();
        let huge = format!(
            "{{\"type\":\"assistant.delta\",\"text\":\"{}\"}}\n",
            "x".repeat(MAX_FRAME_BYTES)
        );
        let events = p.push(&huge);
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .as_ref()
            .err()
            .map(|e| e.contains("max size"))
            .unwrap_or(false));
    }

    #[test]
    fn leftover_incomplete_on_finish_is_error() {
        let mut p = NdjsonFrameParser::new();
        assert!(p
            .push(r#"{"type":"assistant.delta","text":"partial"#)
            .is_empty());
        let finished = p.finish();
        assert_eq!(finished.len(), 1);
        assert!(finished[0]
            .as_ref()
            .err()
            .map(|e| e.contains("incomplete frame"))
            .unwrap_or(false));
    }

    #[test]
    fn rejects_duplicate_operation_frames() {
        let mut p = NdjsonFrameParser::new();
        let line = op_frame("op-1") + "\n";
        assert!(p.push(&line)[0].is_ok());
        let dup = p.push(&line);
        assert!(dup[0].is_err());
        assert!(dup[0]
            .as_ref()
            .err()
            .map(|e| e.contains("duplicate"))
            .unwrap_or(false));
    }

    #[test]
    fn does_not_apply_incomplete_json_line() {
        let mut p = NdjsonFrameParser::new();
        let events = p.push("{\"type\":\"turn.started\"\n");
        assert!(events[0].is_err());
    }

    #[test]
    fn canonical_rejects_bare_app_operation() {
        let mut p = NdjsonFrameParser::new();
        let bare = json!({
            "id": "op-bare",
            "type": "component.update_props",
            "target": {"surfaceId": "s1", "componentId": "c1"},
            "payload": {"maximum": 10}
        })
        .to_string()
            + "\n";
        let events = p.push(&bare);
        assert!(events[0].is_err());
        assert!(p.completed_operations().is_empty());
    }

    #[test]
    fn legacy_compat_harvests_bare_operation_without_newline() {
        // message_cmds post-hoc harvest: buffered raw_text may lack trailing newline.
        let mut p = NdjsonFrameParser::new();
        let bare = json!({
            "id": "op-harvest",
            "type": "component.update_props",
            "target": {"surfaceId": "s1", "componentId": "c1"},
            "payload": {"maximum": 10}
        })
        .to_string();
        assert!(p.push_legacy_compat(&bare).is_empty());
        let finished = p.finish_legacy_compat();
        assert_eq!(finished.len(), 1);
        assert!(finished[0].is_ok());
        assert_eq!(p.completed_operations().len(), 1);
        assert_eq!(p.completed_operations()[0].id, "op-harvest");
    }

    #[test]
    fn incomplete_oversize_halts_further_pushes() {
        let mut p = NdjsonFrameParser::new();
        let huge = "x".repeat(MAX_INCOMPLETE_BUFFER + 1);
        let events = p.push(&huge);
        assert!(events[0]
            .as_ref()
            .err()
            .map(|e| e.contains("incomplete frame"))
            .unwrap_or(false));
        let again = p.push("{\"type\":\"turn.started\",\"turn_id\":\"t1\"}\n");
        assert!(again[0]
            .as_ref()
            .err()
            .map(|e| e.contains("incomplete frame"))
            .unwrap_or(false));
        assert!(p.completed_operations().is_empty());
    }

    #[test]
    fn legacy_agent_response_duplicate_is_atomic() {
        let mut p = NdjsonFrameParser::new();
        let ok = op_frame("op-keep") + "\n";
        assert!(p.push_legacy_compat(&ok)[0].is_ok());
        assert_eq!(p.completed_operations().len(), 1);

        let dup_batch = json!({
            "schemaVersion": "2",
            "assistantMessage": "x",
            "operations": [
                {
                    "id": "op-new",
                    "type": "component.update_props",
                    "target": {"surfaceId": "s1", "componentId": "c1"},
                    "payload": {"maximum": 1}
                },
                {
                    "id": "op-keep",
                    "type": "component.update_props",
                    "target": {"surfaceId": "s1", "componentId": "c1"},
                    "payload": {"maximum": 2}
                }
            ]
        })
        .to_string()
            + "\n";
        let events = p.push_legacy_compat(&dup_batch);
        assert!(events[0]
            .as_ref()
            .err()
            .map(|e| e.contains("duplicate"))
            .unwrap_or(false));
        // Failed batch must not poison op-new or drop op-keep.
        assert_eq!(p.completed_operations().len(), 1);
        assert_eq!(p.completed_operations()[0].id, "op-keep");
        let retry = json!({
            "id": "op-new",
            "type": "component.update_props",
            "target": {"surfaceId": "s1", "componentId": "c1"},
            "payload": {"maximum": 3}
        })
        .to_string()
            + "\n";
        assert!(p.push_legacy_compat(&retry)[0].is_ok());
        assert_eq!(p.completed_operations().len(), 2);
    }

    #[test]
    fn truncate_respects_utf8_char_boundaries() {
        // Emoji (4 bytes), CJK (3 bytes), combining-mark sequence.
        assert_eq!(truncate("hi", 10), "hi");
        assert_eq!(truncate("你好世界", 2), "…"); // first CJK char needs 3 bytes
        assert_eq!(truncate("你好世界", 3), "你…");
        assert_eq!(truncate("你好世界", 6), "你好…");
        assert_eq!(truncate("hello🎉world", 7), "hello…"); // 🎉 starts at byte 5
        assert_eq!(truncate("a\u{0301}b", 2), "a…"); // combining acute starts at byte 1
    }

    #[test]
    fn typed_parse_errors_classify_fatal_vs_incomplete() {
        let incomplete = stream_err(StreamParseErrorKind::Incomplete, "still buffering");
        let (kind, fatal) = StreamParseError::classify_message(&incomplete);
        assert_eq!(kind, StreamParseErrorKind::Incomplete);
        assert!(!fatal);

        let limit = stream_err(StreamParseErrorKind::LimitExceeded, "too big");
        let (kind, fatal) = StreamParseError::classify_message(&limit);
        assert_eq!(kind, StreamParseErrorKind::LimitExceeded);
        assert!(fatal);
    }
}
