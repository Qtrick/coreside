use serde_json::Value;

use super::errors::CrawlerError;
use super::models::{ProtocolEnvelope, ProtocolRequest, PROTOCOL_VERSION, TERMINAL_EVENT_TYPES};

/// Serialize a protocol request as a single NDJSON line (no trailing newline).
pub fn encode_request(req: &ProtocolRequest) -> Result<String, CrawlerError> {
    serde_json::to_string(req).map_err(|e| CrawlerError::Protocol(e.to_string()))
}

/// Parse one stdout line into an envelope. Empty / malformed lines return `Ok(None)` (never panic).
pub fn parse_event_line(line: &str) -> Result<Option<ProtocolEnvelope>, CrawlerError> {
    let raw = line.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let value: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !value.is_object() {
        return Ok(None);
    }
    match serde_json::from_value::<ProtocolEnvelope>(value) {
        Ok(env) => Ok(Some(env)),
        Err(_) => Ok(None),
    }
}

/// Validate protocol version; returns an error message when mismatched.
pub fn validate_version(envelope: &ProtocolEnvelope) -> Result<(), CrawlerError> {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(CrawlerError::Protocol(format!(
            "unsupported protocolVersion: {:?}",
            envelope.protocol_version
        )));
    }
    Ok(())
}

pub fn is_terminal_event(event_type: &str) -> bool {
    TERMINAL_EVENT_TYPES.iter().any(|t| *t == event_type)
}

/// Ensure a terminal event belongs to the expected request id.
pub fn matches_request_id(envelope: &ProtocolEnvelope, request_id: &str) -> bool {
    envelope
        .request_id
        .as_deref()
        .map(|id| id == request_id)
        .unwrap_or(false)
}

pub fn terminal_payload_result(envelope: &ProtocolEnvelope) -> Result<Value, CrawlerError> {
    match envelope.event_type.as_str() {
        "completed" => Ok(envelope.payload.clone()),
        "cancelled" => Err(CrawlerError::Cancelled),
        "failed" => {
            let message = envelope
                .payload
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("sidecar request failed");
            let category = envelope
                .payload
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("sidecar");
            Err(CrawlerError::Sidecar(format!("{category}: {message}")))
        }
        other => Err(CrawlerError::Protocol(format!(
            "unexpected terminal event: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_valid_completed_event() {
        let line = r#"{"protocolVersion":"1","requestId":"r1","type":"completed","payload":{"ok":true}}"#;
        let env = parse_event_line(line).unwrap().unwrap();
        assert_eq!(env.protocol_version, "1");
        assert_eq!(env.request_id.as_deref(), Some("r1"));
        assert_eq!(env.event_type, "completed");
        validate_version(&env).unwrap();
        assert!(matches_request_id(&env, "r1"));
        assert!(!matches_request_id(&env, "other"));
        let payload = terminal_payload_result(&env).unwrap();
        assert_eq!(payload.get("ok").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn malformed_lines_do_not_panic() {
        assert!(parse_event_line("").unwrap().is_none());
        assert!(parse_event_line("   ").unwrap().is_none());
        assert!(parse_event_line("not-json").unwrap().is_none());
        assert!(parse_event_line("[]").unwrap().is_none());
        assert!(parse_event_line("null").unwrap().is_none());
        assert!(parse_event_line("{").unwrap().is_none());
    }

    #[test]
    fn rejects_wrong_protocol_version() {
        let env = ProtocolEnvelope {
            protocol_version: "99".into(),
            request_id: Some("x".into()),
            event_type: "completed".into(),
            payload: json!({}),
        };
        assert!(validate_version(&env).is_err());
    }

    #[test]
    fn encode_request_is_camel_case() {
        let req = ProtocolRequest::new("abc", "health", json!({}));
        let line = encode_request(&req).unwrap();
        assert!(line.contains("\"protocolVersion\":\"1\""));
        assert!(line.contains("\"requestId\":\"abc\""));
        assert!(line.contains("\"type\":\"health\""));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn failed_event_maps_to_sidecar_error() {
        let env = ProtocolEnvelope {
            protocol_version: "1".into(),
            request_id: Some("r".into()),
            event_type: "failed".into(),
            payload: json!({"category":"invalid_request","message":"url is required"}),
        };
        let err = terminal_payload_result(&env).unwrap_err();
        assert_eq!(err.code(), "sidecar");
        assert!(err.to_string().contains("url is required"));
    }
}
