//! Provider-facing progressive operation protocol (`coreside.ops.v1`).
//!
//! This is the authoritative wire format advertised in `response_rules.md`.
//! It is distinct from internal UI [`super::streaming::StreamEvent`] envelopes
//! (`turn.started`, `operation.frame_completed`, …). Those remain Channel/UI
//! events and must not be confused with provider NDJSON.
//!
//! Production progressive mode accepts only:
//!   start → zero-or-more op → exactly one complete|abort
//! Speculative ops update preview only. Durable apply requires a valid
//! terminal (`complete`) plus approval policy — never an incomplete stream.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::limits::{MAX_DEFINITION_JSON_BYTES, MAX_OPERATIONS_PER_TURN, MAX_PATCH_QUEUE_BYTES};
use super::operations::AppOperation;
use super::streaming::{stream_err, StreamParseErrorKind};

/// Wire protocol version string — must match the prompt and capability negotiation.
pub const PROGRESSIVE_OPS_V: &str = "coreside.ops.v1";
/// Preferred AppOperation schema version for progressive groups.
pub const PROGRESSIVE_SCHEMA_VERSION: &str = "2";

pub const MAX_FRAME_BYTES: usize = MAX_DEFINITION_JSON_BYTES;
pub const MAX_INCOMPLETE_BUFFER: usize = MAX_FRAME_BYTES;
pub const MAX_TOTAL_STREAM_BYTES: usize = MAX_PATCH_QUEUE_BYTES;
pub const MAX_EVENTS: usize = 512;
pub const MAX_OPERATIONS: usize = MAX_OPERATIONS_PER_TURN;

/// Expected binding for one progressive stream (from the turn claim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressiveOpsExpect {
    pub turn_id: Option<String>,
    pub attempt_id: Option<String>,
    pub group_id: Option<String>,
    pub schema_version: String,
    pub capability_version: Option<String>,
}

impl Default for ProgressiveOpsExpect {
    fn default() -> Self {
        Self {
            turn_id: None,
            attempt_id: None,
            group_id: None,
            schema_version: PROGRESSIVE_SCHEMA_VERSION.into(),
            capability_version: None,
        }
    }
}

/// Provider-facing NDJSON frame (`coreside.ops.v1`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProgressiveOpsFrame {
    #[serde(rename = "start")]
    Start {
        v: String,
        #[serde(rename = "groupId")]
        group_id: String,
        #[serde(rename = "schemaVersion")]
        schema_version: String,
        #[serde(
            rename = "capabilityVersion",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        capability_version: Option<String>,
        #[serde(rename = "turnId", default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
        #[serde(rename = "attemptId", default, skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
    },
    #[serde(rename = "op")]
    Op {
        v: String,
        #[serde(rename = "frameId")]
        frame_id: u64,
        #[serde(rename = "groupId", default, skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,
        op: AppOperation,
    },
    #[serde(rename = "complete")]
    Complete {
        v: String,
        #[serde(rename = "frameId")]
        frame_id: u64,
        #[serde(rename = "groupId", default, skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,
    },
    #[serde(rename = "abort")]
    Abort {
        v: String,
        #[serde(rename = "frameId")]
        frame_id: u64,
        #[serde(rename = "groupId", default, skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl ProgressiveOpsFrame {
    pub fn frame_type(&self) -> &'static str {
        match self {
            Self::Start { .. } => "start",
            Self::Op { .. } => "op",
            Self::Complete { .. } => "complete",
            Self::Abort { .. } => "abort",
        }
    }

    pub fn protocol_v(&self) -> &str {
        match self {
            Self::Start { v, .. }
            | Self::Op { v, .. }
            | Self::Complete { v, .. }
            | Self::Abort { v, .. } => v.as_str(),
        }
    }
}

/// Terminal outcome of a progressive group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressiveTerminal {
    Complete,
    Abort,
}

/// Authoritative progressive-ops parser for the production send path.
#[derive(Debug)]
pub struct ProgressiveOpsParser {
    buffer: Vec<u8>,
    start: usize,
    total_bytes: usize,
    event_count: usize,
    halted: Option<String>,
    expect: ProgressiveOpsExpect,
    started: bool,
    group_id: Option<String>,
    last_frame_id: Option<u64>,
    seen_operation_ids: std::collections::HashSet<String>,
    /// Speculative ops accepted during streaming (preview only).
    speculative_ops: Vec<AppOperation>,
    terminal: Option<ProgressiveTerminal>,
    /// True when abort discarded the speculative set.
    discarded: bool,
}

impl Default for ProgressiveOpsParser {
    fn default() -> Self {
        Self::new(ProgressiveOpsExpect::default())
    }
}

impl ProgressiveOpsParser {
    pub fn new(expect: ProgressiveOpsExpect) -> Self {
        Self {
            buffer: Vec::new(),
            start: 0,
            total_bytes: 0,
            event_count: 0,
            halted: None,
            expect,
            started: false,
            group_id: None,
            last_frame_id: None,
            seen_operation_ids: std::collections::HashSet::new(),
            speculative_ops: Vec::new(),
            terminal: None,
            discarded: false,
        }
    }

    pub fn push(&mut self, chunk: &str) -> Vec<Result<ProgressiveOpsFrame, String>> {
        self.push_bytes(chunk.as_bytes())
    }

    /// Canonical finish: leftover non-whitespace is incomplete; missing terminal
    /// after a start is fatal. Returns any final frame results plus terminal errors.
    pub fn finish(&mut self) -> Vec<Result<ProgressiveOpsFrame, String>> {
        let mut out = Vec::new();
        if let Some(msg) = &self.halted {
            out.push(Err(msg.clone()));
            return out;
        }
        let trimmed = trim_ascii_ws(&self.buffer[self.start..]).to_vec();
        self.buffer.clear();
        self.start = 0;
        if !trimmed.is_empty() {
            let detail = match std::str::from_utf8(&trimmed) {
                Ok(s) => format!("incomplete frame at end of stream: {}", truncate(s, 80)),
                Err(_) => "incomplete frame at end of stream (invalid utf-8)".into(),
            };
            let msg = stream_err(StreamParseErrorKind::Incomplete, detail);
            self.halt_with(&msg);
            // Incomplete trailing data after any speculative ops invalidates the group.
            self.invalidate_group();
            out.push(Err(msg));
            return out;
        }
        if self.started && self.terminal.is_none() {
            let msg = stream_err(
                StreamParseErrorKind::Halted,
                "progressive stream ended without a terminal complete|abort frame",
            );
            self.halt_with(&msg);
            self.invalidate_group();
            out.push(Err(msg));
        }
        out
    }

    pub fn speculative_operations(&self) -> &[AppOperation] {
        &self.speculative_ops
    }

    /// Operations authorized for durable apply — only after a valid `complete`.
    pub fn durable_operations(&self) -> Option<&[AppOperation]> {
        if self.discarded || self.halted.is_some() {
            return None;
        }
        match self.terminal {
            Some(ProgressiveTerminal::Complete) => Some(&self.speculative_ops),
            _ => None,
        }
    }

    pub fn terminal(&self) -> Option<ProgressiveTerminal> {
        self.terminal
    }

    pub fn is_halted(&self) -> bool {
        self.halted.is_some()
    }

    pub fn halted_reason(&self) -> Option<&str> {
        self.halted.as_deref()
    }

    pub fn discarded(&self) -> bool {
        self.discarded
    }

    pub fn started(&self) -> bool {
        self.started
    }

    /// Fail the entire group (e.g. a sibling op rejected during speculative paint).
    /// Clears speculative ops so durable apply cannot proceed.
    pub fn fail_group(&mut self, detail: impl Into<String>) {
        let _ = self.fatal(detail);
    }

    fn push_bytes(&mut self, chunk: &[u8]) -> Vec<Result<ProgressiveOpsFrame, String>> {
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
            self.halt_with(&msg);
            self.invalidate_group();
            out.push(Err(msg));
            return out;
        }
        self.total_bytes = next_total;
        self.buffer.extend_from_slice(chunk);

        while let Some(rel) = self.buffer[self.start..].iter().position(|&b| b == b'\n') {
            let line_end = self.start + rel;
            let frame_len = line_end - self.start;
            if frame_len == 0 {
                self.start = line_end + 1;
                continue;
            }
            if frame_len > MAX_FRAME_BYTES {
                let msg = stream_err(
                    StreamParseErrorKind::LimitExceeded,
                    format!("frame exceeds max ({MAX_FRAME_BYTES} bytes)"),
                );
                self.halt_with(&msg);
                self.invalidate_group();
                out.push(Err(msg));
                return out;
            }
            let line = match std::str::from_utf8(&self.buffer[self.start..line_end]) {
                Ok(s) => s.trim().to_string(),
                Err(_) => {
                    let msg = stream_err(
                        StreamParseErrorKind::Utf8,
                        "invalid UTF-8 in progressive frame",
                    );
                    self.halt_with(&msg);
                    self.invalidate_group();
                    out.push(Err(msg));
                    return out;
                }
            };
            self.start = line_end + 1;
            if line.is_empty() {
                continue;
            }
            self.event_count = self.event_count.saturating_add(1);
            if self.event_count > MAX_EVENTS {
                let msg = stream_err(
                    StreamParseErrorKind::LimitExceeded,
                    format!("exceeded max events ({MAX_EVENTS})"),
                );
                self.halt_with(&msg);
                self.invalidate_group();
                out.push(Err(msg));
                return out;
            }
            out.push(self.parse_line(&line));
            if self.halted.is_some() {
                return out;
            }
        }

        let incomplete = self.buffer.len() - self.start;
        if incomplete > MAX_INCOMPLETE_BUFFER {
            let msg = stream_err(
                StreamParseErrorKind::LimitExceeded,
                format!("incomplete frame exceeds max buffer ({MAX_INCOMPLETE_BUFFER} bytes)"),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            out.push(Err(msg));
            return out;
        }
        self.compact_if_needed();
        out
    }

    fn parse_line(&mut self, line: &str) -> Result<ProgressiveOpsFrame, String> {
        if self.terminal.is_some() {
            let msg = stream_err(
                StreamParseErrorKind::Halted,
                "frame after progressive terminal",
            );
            self.halt_with(&msg);
            self.invalidate_group();
            return Err(msg);
        }
        if !line.starts_with('{') || !line.ends_with('}') {
            // Complete NDJSON line that isn't a JSON object is malformed (not buffering).
            let msg = stream_err(
                StreamParseErrorKind::Malformed,
                format!("malformed progressive frame: {}", truncate(line, 80)),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            return Err(msg);
        }
        let value: Value = serde_json::from_str(line).map_err(|e| {
            let msg = stream_err(
                StreamParseErrorKind::Malformed,
                format!("malformed frame: {e}"),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            msg
        })?;

        // Reject unknown mandatory frame types early (not StreamEvent names).
        let type_tag = value.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !matches!(type_tag, "start" | "op" | "complete" | "abort") {
            let msg = stream_err(
                StreamParseErrorKind::Unrecognized,
                format!("unknown mandatory progressive frame type: {type_tag}"),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            return Err(msg);
        }

        let frame: ProgressiveOpsFrame = serde_json::from_value(value).map_err(|e| {
            let msg = stream_err(
                StreamParseErrorKind::Malformed,
                format!("progressive frame deserialize: {e}"),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            msg
        })?;

        if frame.protocol_v() != PROGRESSIVE_OPS_V {
            let msg = stream_err(
                StreamParseErrorKind::Unrecognized,
                format!("unsupported progressive protocol v={}", frame.protocol_v()),
            );
            self.halt_with(&msg);
            self.invalidate_group();
            return Err(msg);
        }

        self.apply_frame(frame.clone())?;
        Ok(frame)
    }

    fn apply_frame(&mut self, frame: ProgressiveOpsFrame) -> Result<(), String> {
        match frame {
            ProgressiveOpsFrame::Start {
                group_id,
                schema_version,
                capability_version,
                turn_id,
                attempt_id,
                ..
            } => {
                if self.started {
                    return self.fatal("duplicate progressive start frame");
                }
                if schema_version != self.expect.schema_version {
                    return self.fatal(format!(
                        "schemaVersion mismatch: got {schema_version}, expected {}",
                        self.expect.schema_version
                    ));
                }
                if let Some(exp) = &self.expect.group_id {
                    if &group_id != exp {
                        return self
                            .fatal(format!("groupId mismatch: got {group_id}, expected {exp}"));
                    }
                }
                if let (Some(exp), Some(got)) = (&self.expect.turn_id, turn_id.as_ref()) {
                    if got != exp {
                        return self.fatal(format!("turnId mismatch: got {got}, expected {exp}"));
                    }
                }
                if let (Some(exp), Some(got)) = (&self.expect.attempt_id, attempt_id.as_ref()) {
                    if got != exp {
                        return self
                            .fatal(format!("attemptId mismatch: got {got}, expected {exp}"));
                    }
                }
                if let (Some(exp), Some(got)) =
                    (&self.expect.capability_version, capability_version.as_ref())
                {
                    if got != exp {
                        return self.fatal(format!(
                            "capabilityVersion mismatch: got {got}, expected {exp}"
                        ));
                    }
                }
                self.started = true;
                self.group_id = Some(group_id);
                Ok(())
            }
            ProgressiveOpsFrame::Op {
                frame_id,
                group_id,
                op,
                ..
            } => {
                if !self.started {
                    return self.fatal("progressive op before start");
                }
                self.check_group(group_id.as_deref())?;
                self.check_frame_id(frame_id)?;
                if self.speculative_ops.len() >= MAX_OPERATIONS {
                    return self.fatal(format!("operations exceed max of {MAX_OPERATIONS}"));
                }
                if !self.seen_operation_ids.insert(op.id.clone()) {
                    return self.fatal(format!("duplicate or mutated operation id: {}", op.id));
                }
                self.speculative_ops.push(op);
                Ok(())
            }
            ProgressiveOpsFrame::Complete {
                frame_id, group_id, ..
            } => {
                if !self.started {
                    return self.fatal("progressive complete before start");
                }
                self.check_group(group_id.as_deref())?;
                self.check_frame_id(frame_id)?;
                self.terminal = Some(ProgressiveTerminal::Complete);
                Ok(())
            }
            ProgressiveOpsFrame::Abort {
                frame_id, group_id, ..
            } => {
                if !self.started {
                    return self.fatal("progressive abort before start");
                }
                self.check_group(group_id.as_deref())?;
                self.check_frame_id(frame_id)?;
                self.terminal = Some(ProgressiveTerminal::Abort);
                self.discarded = true;
                self.speculative_ops.clear();
                self.seen_operation_ids.clear();
                Ok(())
            }
        }
    }

    fn check_group(&mut self, group_id: Option<&str>) -> Result<(), String> {
        if let Some(got) = group_id {
            if let Some(exp) = &self.group_id {
                if got != exp {
                    return self.fatal(format!("wrong groupId: got {got}, expected {exp}"));
                }
            }
        }
        Ok(())
    }

    fn check_frame_id(&mut self, frame_id: u64) -> Result<(), String> {
        match self.last_frame_id {
            None => {
                // First numbered frame after start may be any u64; later frames must increase.
                self.last_frame_id = Some(frame_id);
                Ok(())
            }
            Some(prev) => {
                if frame_id <= prev {
                    return self.fatal(format!(
                        "frameId not monotonically increasing: got {frame_id}, previous {prev}"
                    ));
                }
                self.last_frame_id = Some(frame_id);
                Ok(())
            }
        }
    }

    fn fatal(&mut self, detail: impl Into<String>) -> Result<(), String> {
        let msg = stream_err(StreamParseErrorKind::Halted, detail);
        self.halt_with(&msg);
        self.invalidate_group();
        Err(msg)
    }

    fn halt_with(&mut self, msg: &str) {
        if self.halted.is_none() {
            self.halted = Some(msg.to_string());
        }
    }

    fn invalidate_group(&mut self) {
        self.discarded = true;
        self.speculative_ops.clear();
        self.seen_operation_ids.clear();
        // Leave terminal as-is if already set; otherwise ensure durable_ops is None.
        if self.terminal == Some(ProgressiveTerminal::Complete) {
            self.terminal = None;
        }
    }

    fn compact_if_needed(&mut self) {
        if self.start > 0 && self.start >= self.buffer.len() / 2 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
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
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= n)
        .last()
        .unwrap_or(0);
    format!("{}…", &s[..end])
}

/// Reconcile a final aggregate `operations` array with the progressive preview set.
/// Exact match by ordered operation id + type; any difference fails closed.
pub fn reconcile_final_operations(
    previewed: &[AppOperation],
    final_ops: &[AppOperation],
) -> Result<(), String> {
    if previewed.len() != final_ops.len() {
        return Err(format!(
            "final operations length {} differs from progressive preview {}",
            final_ops.len(),
            previewed.len()
        ));
    }
    for (i, (a, b)) in previewed.iter().zip(final_ops.iter()).enumerate() {
        if a.id != b.id || a.op_type != b.op_type {
            return Err(format!(
                "final operation[{i}] diverges from progressive preview (id/type)"
            ));
        }
        // Payload must match exactly — silent replacement is forbidden.
        if a.payload != b.payload || a.target != b.target {
            return Err(format!(
                "final operation[{i}] payload/target differs from progressive preview"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn start_line(group: &str) -> String {
        json!({
            "v": PROGRESSIVE_OPS_V,
            "type": "start",
            "groupId": group,
            "schemaVersion": "2",
            "capabilityVersion": "1"
        })
        .to_string()
            + "\n"
    }

    fn op_line(group: &str, frame_id: u64, op_id: &str) -> String {
        json!({
            "v": PROGRESSIVE_OPS_V,
            "type": "op",
            "frameId": frame_id,
            "groupId": group,
            "op": {
                "id": op_id,
                "type": "component.update_props",
                "target": {"surfaceId": "s1", "componentId": "c1"},
                "payload": {"maximum": 10}
            }
        })
        .to_string()
            + "\n"
    }

    fn complete_line(group: &str, frame_id: u64) -> String {
        json!({
            "v": PROGRESSIVE_OPS_V,
            "type": "complete",
            "frameId": frame_id,
            "groupId": group
        })
        .to_string()
            + "\n"
    }

    fn abort_line(group: &str, frame_id: u64) -> String {
        json!({
            "v": PROGRESSIVE_OPS_V,
            "type": "abort",
            "frameId": frame_id,
            "groupId": group,
            "reason": "cancelled"
        })
        .to_string()
            + "\n"
    }

    fn parser() -> ProgressiveOpsParser {
        ProgressiveOpsParser::new(ProgressiveOpsExpect {
            capability_version: Some("1".into()),
            ..Default::default()
        })
    }

    #[test]
    fn valid_start_op_complete() {
        let mut p = parser();
        assert!(p.push(&start_line("g1")).iter().all(|e| e.is_ok()));
        assert!(p.push(&op_line("g1", 1, "op-1")).iter().all(|e| e.is_ok()));
        assert!(p.push(&complete_line("g1", 2)).iter().all(|e| e.is_ok()));
        assert!(p.finish().is_empty());
        assert_eq!(p.durable_operations().map(|o| o.len()), Some(1));
    }

    #[test]
    fn valid_abort_discards_ops() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let _ = p.push(&abort_line("g1", 2));
        assert!(p.finish().is_empty());
        assert!(p.durable_operations().is_none());
        assert!(p.discarded());
        assert!(p.speculative_operations().is_empty());
    }

    #[test]
    fn missing_start_rejects_op() {
        let mut p = parser();
        let ev = p.push(&op_line("g1", 1, "op-1"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn missing_terminal_on_finish() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let fin = p.finish();
        assert!(fin.iter().any(|e| e.is_err()));
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn duplicate_terminal_rejected() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&complete_line("g1", 1));
        let ev = p.push(&complete_line("g1", 2));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn trailing_data_after_terminal() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let _ = p.push(&complete_line("g1", 2));
        let ev = p.push(&op_line("g1", 3, "op-2"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn wrong_group_id() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let ev = p.push(&op_line("g2", 1, "op-1"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn out_of_order_frame_ids() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 2, "op-1"));
        let ev = p.push(&op_line("g1", 1, "op-2"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn duplicate_frame_ids() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let ev = p.push(&op_line("g1", 1, "op-2"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn malformed_json_after_valid_op() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let ev = p.push("{\"v\":\"coreside.ops.v1\",\"type\":\"op\",broken\n");
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
        assert!(p.speculative_operations().is_empty());
    }

    #[test]
    fn incomplete_final_frame_after_valid_op() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let _ = p.push("{\"v\":\"coreside.ops.v1\",\"type\":\"complete\"");
        let fin = p.finish();
        assert!(fin.iter().any(|e| e.is_err()));
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn unknown_mandatory_frame() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let bad =
            r#"{"v":"coreside.ops.v1","type":"turn.started","turn_id":"t1"}"#.to_string() + "\n";
        let ev = p.push(&bad);
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
    }

    #[test]
    fn rejected_second_op_invalidates_group() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        // Duplicate operation id
        let ev = p.push(&op_line("g1", 2, "op-1"));
        assert!(ev[0].is_err());
        assert!(p.durable_operations().is_none());
        assert!(p.speculative_operations().is_empty());
    }

    #[test]
    fn reconcile_rejects_divergent_final() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        let _ = p.push(&complete_line("g1", 2));
        let previewed = p.durable_operations().unwrap().to_vec();
        let mut other = previewed.clone();
        other[0].payload = json!({"maximum": 99});
        assert!(reconcile_final_operations(&previewed, &other).is_err());
        assert!(reconcile_final_operations(&previewed, &previewed).is_ok());
    }

    #[test]
    fn turn_attempt_binding() {
        let mut p = ProgressiveOpsParser::new(ProgressiveOpsExpect {
            turn_id: Some("turn-1".into()),
            attempt_id: Some("attempt-1".into()),
            schema_version: "2".into(),
            capability_version: Some("1".into()),
            group_id: None,
        });
        let bad = json!({
            "v": PROGRESSIVE_OPS_V,
            "type": "start",
            "groupId": "g1",
            "schemaVersion": "2",
            "capabilityVersion": "1",
            "turnId": "turn-other",
            "attemptId": "attempt-1"
        })
        .to_string()
            + "\n";
        assert!(p.push(&bad)[0].is_err());
    }

    #[test]
    fn fail_group_clears_durable_ops() {
        let mut p = parser();
        let _ = p.push(&start_line("g1"));
        let _ = p.push(&op_line("g1", 1, "op-1"));
        p.fail_group("rejected sibling");
        assert!(p.durable_operations().is_none());
        assert!(p.speculative_operations().is_empty());
        assert!(p.is_halted());
    }
}
