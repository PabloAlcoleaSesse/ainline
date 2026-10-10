pub mod antigravity;
pub mod claude;
pub mod copilot;
pub mod ollama;
pub mod openai;
use crate::config::{Kind, Model};
use std::{
    ffi::OsString,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("provider exited with {code:?}: {message}")]
    Failed { code: Option<i32>, message: String },
    #[error("provider interrupted")]
    Interrupted,
    #[error("provider argument {0} changes the request protocol; use ai flags instead")]
    Argument(String),
    #[error("invalid provider response: {0}")]
    Response(String),
}
/// Adapter builds argv without invoking a shell. Context is delivered via stdin.
pub trait Provider {
    fn command(&self, binary: &Path, model: &str) -> Command;
    fn input(&self, model: &str, prompt: &str) -> String {
        let _ = model;
        prompt.to_owned()
    }
    fn output(&self, bytes: Vec<u8>) -> Result<Vec<u8>, ProviderError> {
        Ok(bytes)
    }
}
pub fn adapter(kind: Kind) -> Box<dyn Provider> {
    match kind {
        Kind::Antigravity => Box::new(antigravity::Antigravity),
        Kind::Copilot => Box::new(copilot::Copilot),
        Kind::Claude => Box::new(claude::Claude),
        Kind::Ollama => Box::new(ollama::Ollama),
        Kind::Openai => Box::new(openai::OpenAi),
    }
}
pub fn execute(
    model: &Model,
    binary: &Path,
    prompt: &str,
    cwd: &Path,
) -> Result<Vec<u8>, ProviderError> {
    execute_with_args(model, binary, prompt, cwd, &[])
}

/// Keep the stdin/output protocol owned by ai while allowing native tuning flags.
pub fn validate_args(args: &[OsString]) -> Result<(), ProviderError> {
    for arg in args {
        let text = arg.to_string_lossy();
        let key = text.split('=').next().unwrap_or_default();
        // Short options may carry attached values (-pPROMPT) or form clusters.
        // Reserve these letters in any short-option token to avoid bypasses.
        let reserved_short = text.starts_with('-')
            && !text.starts_with("--")
            && text[1..].chars().any(|c| "pimhvcr".contains(c));
        if reserved_short
            || matches!(
                key,
                "--" | "-p"
                    | "--prompt"
                    | "--print"
                    | "-i"
                    | "--interactive"
                    | "--prompt-interactive"
                    | "--input-format"
                    | "--output-format"
                    | "--model"
                    | "-m"
                    | "--help"
                    | "-h"
                    | "--version"
                    | "-v"
                    | "--continue"
                    | "-c"
                    | "--resume"
                    | "-r"
                    | "--conversation"
                    | "--session-id"
            )
        {
            return Err(ProviderError::Argument(text.into_owned()));
        }
    }
    Ok(())
}

pub fn execute_with_args(
    model: &Model,
    binary: &Path,
    prompt: &str,
    cwd: &Path,
    args: &[OsString],
) -> Result<Vec<u8>, ProviderError> {
    validate_args(args)?;
    let provider = adapter(model.provider);
    let input = provider.input(&model.model, prompt);
    let mut command = provider.command(binary, &model.model);
    command.args(args);
    let mut child = command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    // Write concurrently: a child may emit output before consuming the whole request.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output()?;
    let written = writer
        .join()
        .map_err(|_| ProviderError::Response("stdin writer panicked".into()))?;
    if output.status.code().is_none() || matches!(output.status.code(), Some(130 | 143)) {
        return Err(ProviderError::Interrupted);
    }
    // Some CLIs encode cancellation in JSON while returning a generic exit code.
    let decoded = provider.output(output.stdout);
    if matches!(&decoded, Err(ProviderError::Interrupted)) {
        return decoded;
    }
    if !output.status.success() {
        return Err(ProviderError::Failed {
            code: output.status.code(),
            message: String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect(),
        });
    }
    // A successful child can still report useful warnings or permission notices.
    std::io::stderr().lock().write_all(&output.stderr)?;
    written?;
    decoded
}
