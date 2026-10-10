use super::{Provider, ProviderError};
use std::{path::Path, process::Command};
pub struct OpenAi;
impl Provider for OpenAi {
    fn command(&self, binary: &Path, _model: &str) -> Command {
        let mut c = Command::new(binary);
        c.args(["responses", "create"]);
        c
    }
    fn input(&self, model: &str, prompt: &str) -> String {
        // JSON is a YAML subset accepted by the official CLI's request-body stdin.
        serde_json::json!({"model": model, "input": prompt}).to_string()
    }
    fn output(&self, bytes: Vec<u8>) -> Result<Vec<u8>, ProviderError> {
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| ProviderError::Response(e.to_string()))?;
        if value["status"] != "completed" {
            return Err(ProviderError::Response(format!(
                "response status: {}",
                value["status"]
            )));
        }
        let mut text = String::new();
        if let Some(items) = value["output"].as_array() {
            for item in items {
                if item["type"] != "message" {
                    continue;
                }
                if let Some(parts) = item["content"].as_array() {
                    for part in parts {
                        if let Some(t) = part["text"].as_str().or_else(|| part["refusal"].as_str())
                        {
                            text.push_str(t);
                        }
                    }
                }
            }
        }
        if text.is_empty() {
            return Err(ProviderError::Response("no assistant text".into()));
        }
        text.push('\n');
        Ok(text.into_bytes())
    }
}
