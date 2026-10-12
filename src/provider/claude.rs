use super::Provider;
use std::{path::Path, process::Command};
pub struct Claude;
impl Provider for Claude {
    fn command(&self, binary: &Path, model: &str) -> Command {
        let mut c = Command::new(binary);
        // Read-only assistant: no agent tools, hence retries cannot duplicate edits.
        c.args([
            "--print",
            "--model",
            model,
            "--output-format",
            "text",
            "--tools",
            "",
            "--no-session-persistence",
        ]);
        c
    }
}

/// Decode only assistant text deltas; omit tool events, metadata and duplicate result text.
#[derive(Default)]
pub struct StreamDecoder {
    pending: Vec<u8>,
    emitted: bool,
    completed: bool,
    error: Option<super::ProviderError>,
}
impl StreamDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, super::ProviderError> {
        self.pending.extend_from_slice(bytes);
        let mut chunks = Vec::new();
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<_> = self.pending.drain(..=end).collect();
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let event: serde_json::Value = serde_json::from_slice(&line).map_err(|e| {
                super::ProviderError::Response(format!("invalid Claude stream event: {e}"))
            })?;
            if self.completed {
                return Err(super::ProviderError::Response(
                    "Claude emitted data after its final result".into(),
                ));
            }
            if event["type"] == "stream_event"
                && event["event"]["type"] == "content_block_delta"
                && event["event"]["delta"]["type"] == "text_delta"
                && event["parent_tool_use_id"].is_null()
            {
                let text = event["event"]["delta"]["text"].as_str().ok_or_else(|| {
                    super::ProviderError::Response("Claude text delta is missing text".into())
                })?;
                self.emitted = true;
                chunks.push(text.as_bytes().to_vec());
            } else if event["type"] == "result" {
                self.completed = true;
                let is_error = event["is_error"].as_bool().ok_or_else(|| {
                    super::ProviderError::Response(
                        "Claude result is missing its error status".into(),
                    )
                })?;
                if is_error || event["subtype"].as_str().is_some_and(|s| s != "success") {
                    let message = event["result"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| event["errors"].to_string());
                    self.error = Some(super::ProviderError::Response(format!(
                        "Claude request failed: {message}"
                    )));
                } else if !self.emitted
                    && let Some(text) = event["result"].as_str()
                {
                    chunks.push(text.as_bytes().to_vec());
                }
            }
        }
        Ok(chunks)
    }
    pub fn finish(self) -> Result<(), super::ProviderError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.completed || self.pending.iter().any(|b| !b.is_ascii_whitespace()) {
            return Err(super::ProviderError::Response(
                "incomplete Claude stream (missing final result or newline)".into(),
            ));
        }
        Ok(())
    }
}

pub fn streaming_command(binary: &Path, model: &str) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "--print",
        "--model",
        model,
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",
        "--no-session-persistence",
    ]);
    command
}
