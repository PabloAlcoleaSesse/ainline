use super::Provider;
use std::{path::Path, process::Command};
pub struct Ollama;
impl Provider for Ollama {
    fn command(&self, binary: &Path, model: &str) -> Command {
        let mut c = Command::new(binary);
        c.args(["run", model]);
        c
    }
}
