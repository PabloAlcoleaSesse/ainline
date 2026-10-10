use super::{Provider, ProviderError};
use serde_json::Value;
use std::{path::Path, process::Command};

pub struct Antigravity;

impl Provider for Antigravity {
    fn command(&self, binary: &Path, model: &str) -> Command {
        let mut command = Command::new(binary);
        command.args([
            "--model",
            model,
            "--mode",
            "plan",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--print-timeout",
            "5m",
        ]);
        command
    }

    fn input(&self, _model: &str, prompt: &str) -> String {
        let message = serde_json::json!({"event": "user", "message": {"content": prompt}});
        format!("{message}\n")
    }

    fn output(&self, bytes: Vec<u8>) -> Result<Vec<u8>, ProviderError> {
        let text =
            std::str::from_utf8(&bytes).map_err(|e| ProviderError::Response(e.to_string()))?;
        let mut response = None;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let event: Value = serde_json::from_str(line)
                .map_err(|e| ProviderError::Response(format!("invalid Antigravity event: {e}")))?;
            if event["event"] != "result" {
                continue;
            }
            let result = &event["result"];
            match result["status"].as_str() {
                Some("CANCELED" | "INTERRUPTED") => return Err(ProviderError::Interrupted),
                Some("SUCCESS") => {
                    if response.is_some() {
                        return Err(ProviderError::Response(
                            "multiple Antigravity results for one request".into(),
                        ));
                    }
                    let text = result["response"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .ok_or_else(|| {
                            ProviderError::Response("Antigravity returned no text".into())
                        })?;
                    response = Some(text.as_bytes().to_vec());
                }
                status => {
                    return Err(ProviderError::Response(format!(
                        "Antigravity status {}: {}",
                        status.unwrap_or("missing"),
                        result["error"]
                            .as_str()
                            .unwrap_or("request did not complete")
                    )));
                }
            }
        }
        response.ok_or_else(|| ProviderError::Response("missing Antigravity result event".into()))
    }
}
