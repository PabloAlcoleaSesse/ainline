pub mod antigravity;
pub mod claude;
pub mod copilot;
pub mod ollama;
pub mod openai;
pub mod process;
use crate::config::{Kind, Model};
use std::{ffi::OsString, path::Path, process::Command};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("provider exited with {code:?}: {message}")]
    Failed { code: Option<i32>, message: String },
    #[error("provider timed out")]
    Timeout,
    #[error("provider output exceeded {0} bytes")]
    OutputLimit(u64),
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
            && text[1..].chars().any(|c| "pimhvcro".contains(c));
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
                    | "--json"
                    | "--output-last-message"
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
    execute_controlled(
        model,
        binary,
        prompt,
        cwd,
        args,
        process::Limits::default(),
        false,
    )
}

pub fn execute_controlled(
    model: &Model,
    binary: &Path,
    prompt: &str,
    cwd: &Path,
    args: &[OsString],
    limits: process::Limits,
    stream: bool,
) -> Result<Vec<u8>, ProviderError> {
    validate_args(args)?;
    let provider = adapter(model.provider);
    let input = provider.input(&model.model, prompt);
    let mut command = if stream && model.provider == Kind::Claude {
        claude::streaming_command(binary, &model.model)
    } else {
        provider.command(binary, &model.model)
    };
    command.args(args).current_dir(cwd);
    let stream = if !stream || !supports_streaming(model.provider) {
        process::Stream::Buffered
    } else if model.provider == Kind::Claude {
        process::Stream::Claude
    } else {
        process::Stream::Text
    };
    let output = process::run(command, input.into_bytes(), limits, stream)?;
    if output.status.code().is_none() || matches!(output.status.code(), Some(130 | 143)) {
        return Err(ProviderError::Interrupted);
    }
    let diagnostic = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let mut failure_message: String = String::from_utf8_lossy(diagnostic)
        .chars()
        .take(2000)
        .collect();
    if failure_message.is_empty()
        && let Err(error) = &output.stream_result
    {
        failure_message = error.to_string();
    }
    let decoded = provider.output(output.stdout);
    if matches!(&decoded, Err(ProviderError::Interrupted)) {
        return decoded;
    }
    if !output.status.success() {
        return Err(ProviderError::Failed {
            code: output.status.code(),
            message: failure_message,
        });
    }
    output.written?;
    output.stream_result?;
    decoded
}

/// Forward text as emitted by the CLI. Structured adapters validate before printing.
pub fn supports_streaming(kind: Kind) -> bool {
    matches!(kind, Kind::Claude | Kind::Copilot | Kind::Ollama)
}
