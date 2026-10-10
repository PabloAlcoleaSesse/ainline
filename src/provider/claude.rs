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
