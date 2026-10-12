//! OpenAI is exposed through the user's Codex CLI, not the API-oriented `openai` CLI.
use super::Provider;
use std::{path::Path, process::Command};
pub struct OpenAi;
impl Provider for OpenAi {
    fn command(&self, binary: &Path, model: &str) -> Command {
        let mut command = Command::new(binary);
        command.args([
            "--no-daemon",
            "--ask-for-approval",
            "never",
            "exec",
            "--sandbox",
            "read-only",
            "--ephemeral",
            "--skip-git-repo-check",
            "--color",
            "never",
        ]);
        // Leave model selection to Codex unless the user explicitly configured one.
        if model != "default" {
            command.args(["--model", model]);
        }
        // No positional prompt: codex exec reads the assembled request from stdin.
        // Native progress goes to stderr and the final answer goes to stdout.
        command
    }
}
