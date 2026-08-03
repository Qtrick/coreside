//! Progressive validated operation streaming (NDJSON frames).
//!
//! Incomplete JSON is never applied. Frames are buffered until a complete
//! line/object is available, then validated before preview/apply events.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::operations::{AgentResponseV2, AppOperation};

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

#[derive(Debug, Default)]
pub struct NdjsonFrameParser {
    buffer: String,
    seen_operation_ids: std::collections::HashSet<String>,
    completed_operations: Vec<AppOperation>,
}

impl NdjsonFrameParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push raw stream bytes/text. Returns complete parsed events.
    pub fn push(&mut self, chunk: &str) -> Vec<Result<StreamEvent, String>> {
        self.buffer.push_str(chunk);
        let mut out = Vec::new();
        while let Some(idx) = self.buffer.find('\n') {
            let line = self.buffer[..idx].trim().to_string();
            self.buffer = self.buffer[idx + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            out.push(self.parse_line(&line));
        }
        out
    }

    /// Flush any remaining complete JSON object (no trailing newline).
    pub fn finish(&mut self) -> Vec<Result<StreamEvent, String>> {
        let rem = self.buffer.trim().to_string();
        self.buffer.clear();
        if rem.is_empty() {
            return vec![];
        }
        vec![self.parse_line(&rem)]
    }

    fn parse_line(&mut self, line: &str) -> Result<StreamEvent, String> {
        // Incomplete JSON should not reach here as a full line, but guard anyway.
        if !line.starts_with('{') || !line.ends_with('}') {
            return Err(format!(
                "incomplete or invalid frame: {}",
                truncate(line, 80)
            ));
        }
        let value: Value =
            serde_json::from_str(line).map_err(|e| format!("malformed frame: {e}"))?;
        // Accept either a StreamEvent or a bare AppOperation / AgentResponseV2 fragment.
        if let Ok(ev) = serde_json::from_value::<StreamEvent>(value.clone()) {
            if let StreamEvent::OperationFrameCompleted { operation } = &ev {
                if !self.seen_operation_ids.insert(operation.id.clone()) {
                    return Err(format!("duplicate operation frame: {}", operation.id));
                }
                self.completed_operations.push(operation.clone());
            }
            return Ok(ev);
        }
        if let Ok(op) = serde_json::from_value::<AppOperation>(value.clone()) {
            if !self.seen_operation_ids.insert(op.id.clone()) {
                return Err(format!("duplicate operation frame: {}", op.id));
            }
            self.completed_operations.push(op.clone());
            return Ok(StreamEvent::OperationFrameCompleted { operation: op });
        }
        if let Ok(resp) = serde_json::from_value::<AgentResponseV2>(value) {
            for op in &resp.operations {
                self.seen_operation_ids.insert(op.id.clone());
                self.completed_operations.push(op.clone());
            }
            return Ok(StreamEvent::PreviewUpdated {
                operations: resp.operations,
            });
        }
        Err("unrecognized stream frame".into())
    }

    pub fn completed_operations(&self) -> &[AppOperation] {
        &self.completed_operations
    }
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
    fn rejects_duplicate_operation_frames() {
        let mut p = NdjsonFrameParser::new();
        let line = json!({
            "type": "operation.frame_completed",
            "operation": {
                "id": "op-1",
                "type": "component.update_props",
                "target": {"surfaceId": "s1", "componentId": "c1"},
                "payload": {"maximum": 10}
            }
        })
        .to_string()
            + "\n";
        assert!(p.push(&line)[0].is_ok());
        let dup = p.push(&line);
        assert!(dup[0].is_err());
    }

    #[test]
    fn does_not_apply_incomplete_json_line() {
        let mut p = NdjsonFrameParser::new();
        let events = p.push("{\"type\":\"turn.started\"\n");
        assert!(events[0].is_err());
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
}
