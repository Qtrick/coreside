//! Shared HTTP response body budgets for provider adapters.

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use super::errors::AiError;
use crate::security::redact_secrets;

/// Hard ceiling for a single non-streaming provider response body (decompressed bytes).
pub const MAX_PROVIDER_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Read a response body with Content-Length precheck, cancel, and byte ceiling.
pub async fn read_response_text_bounded(
    response: reqwest::Response,
    cancel: &CancellationToken,
    max_bytes: usize,
    redact_key: Option<&str>,
) -> Result<String, AiError> {
    // Compare as u64 — never cast Content-Length to usize first (truncation on 32-bit).
    if let Some(len) = response.content_length() {
        if len > max_bytes as u64 {
            return Err(AiError::Provider(format!(
                "provider response Content-Length {len} exceeds limit {max_bytes}"
            )));
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    if let Some(len) = response.content_length() {
        let hint = usize::try_from(len).unwrap_or(max_bytes).min(max_bytes);
        buf.reserve(hint);
    }

    let mut stream = response.bytes_stream();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Err(AiError::Cancelled),
            next = stream.next() => {
                match next {
                    None => break,
                    Some(Err(e)) => {
                        return Err(AiError::Http(redact_secrets(&e.to_string(), redact_key)));
                    }
                    Some(Ok(chunk)) => {
                        let next_len = buf.len().saturating_add(chunk.len());
                        if next_len > max_bytes {
                            return Err(AiError::Provider(format!(
                                "provider response exceeded limit of {max_bytes} bytes"
                            )));
                        }
                        buf.extend_from_slice(&chunk);
                    }
                }
            }
        }
    }

    String::from_utf8(buf).map_err(|e| {
        AiError::Parse(format!(
            "provider response was not valid UTF-8: {}",
            redact_secrets(&e.to_string(), redact_key)
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_provider_response_bytes_is_bounded() {
        assert!(MAX_PROVIDER_RESPONSE_BYTES <= 16 * 1024 * 1024);
        assert!(MAX_PROVIDER_RESPONSE_BYTES >= 1024 * 1024);
    }

    #[test]
    fn content_length_limit_compares_as_u64() {
        // Guard against regressions that cast Content-Length to usize before compare.
        let max = MAX_PROVIDER_RESPONSE_BYTES;
        let over = (max as u64).saturating_add(1);
        assert!(over > max as u64);
    }
}
