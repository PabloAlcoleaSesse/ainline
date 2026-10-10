use super::Provider;
use std::{path::Path, process::Command};

pub struct Copilot;

impl Provider for Copilot {
    fn command(&self, binary: &Path, model: &str) -> Command {
        let mut command = Command::new(binary);
        // A pipe selects programmatic mode. -p would discard the piped context.
        command.args([
            "--model",
            model,
            "--silent",
            "--output-format",
            "text",
            "--no-ask-user",
            "--no-auto-update",
            "--no-color",
            "--no-custom-instructions",
            "--disable-builtin-mcps",
            // An explicitly empty allowlist disables agent tools.
            "--available-tools",
        ]);
        command
    }
}
