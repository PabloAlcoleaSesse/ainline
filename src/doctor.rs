//! Read-only diagnostics; never submits a model prompt or prints authentication output.
use crate::{
    config::{Config, Kind},
    provider::{
        ProviderError,
        process::{self, Limits},
    },
};
use anyhow::Result;
use serde::Serialize;
use std::{path::Path, process::Command, time::Duration};

#[derive(Debug, thiserror::Error)]
#[error("diagnostic checks failed")]
pub struct Unhealthy;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warning,
    Error,
}
#[derive(Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub message: String,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub checks: Vec<Check>,
    pub healthy: bool,
}
impl Report {
    fn add(&mut self, name: &str, status: Status, message: impl Into<String>) {
        if matches!(status, Status::Error) {
            self.healthy = false;
        }
        self.checks.push(Check {
            name: name.into(),
            status,
            message: message.into(),
        });
    }
    pub fn text(&self) -> String {
        let mut text = String::new();
        for check in &self.checks {
            let label = match check.status {
                Status::Ok => "ok",
                Status::Warning => "warning",
                Status::Error => "error",
            };
            text.push_str(&format!("[{label}] {}: {}\n", check.name, check.message));
        }
        text
    }
}
fn probe(binary: &Path, args: &[&str], timeout: u64) -> Result<Option<process::Output>> {
    let mut command = Command::new(binary);
    command.args(args);
    match process::capture(
        command,
        Limits {
            timeout: Duration::from_secs(timeout),
            max_output_bytes: 65_536,
        },
    ) {
        Ok(output) => Ok(Some(output)),
        Err(ProviderError::Interrupted) => Err(ProviderError::Interrupted.into()),
        Err(_) => Ok(None),
    }
}

pub fn diagnose(
    provider: Option<&str>,
    profile_name: Option<&str>,
    timeout: u64,
) -> Result<Report> {
    let mut report = Report {
        healthy: true,
        ..Report::default()
    };
    // Do not echo parser errors: YAML may contain private instruction text.
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            report.add(
                "config",
                Status::Error,
                "Cannot load configuration; run ai config validate locally for details.",
            );
            return Ok(report);
        }
    };
    report.add(
        "config",
        Status::Ok,
        "Configuration is valid (built-in defaults apply if no file exists).",
    );
    let profile = match profile_name {
        Some(name) => match config.profiles.get(name) {
            Some(profile) => Some(profile),
            None => {
                report.add("profile", Status::Error, "Selected profile does not exist.");
                return Ok(report);
            }
        },
        None => None,
    };
    if let Some(profile) = profile {
        let readable = profile
            .instructions_file
            .as_ref()
            .is_none_or(|p| p.is_file())
            && profile.instructions_text().is_ok();
        report.add(
            "instructions",
            if readable { Status::Ok } else { Status::Error },
            if readable {
                "Instructions are readable."
            } else {
                "Instructions file is missing, unreadable, invalid UTF-8 or exceeds 1 MiB."
            },
        );
    }
    let id = provider
        .or_else(|| profile.and_then(|p| p.provider.as_deref()))
        .unwrap_or(&config.default_provider);
    let mut selected = match config.resolve(id) {
        Ok(model) => model,
        Err(_) => {
            report.add(
                "provider",
                Status::Error,
                "Unknown provider; run ai status to list configured providers.",
            );
            return Ok(report);
        }
    };
    if provider.is_none()
        && let Some(model) = profile.and_then(|p| p.model.as_deref())
    {
        selected.model = config.resolve_model(selected.provider, model)?;
    }
    report.add(
        "provider",
        Status::Ok,
        format!("{id}, model {}", selected.model),
    );
    let binary = match which::which(&selected.binary) {
        Ok(path) => path,
        Err(_) => {
            report.add(
                "binary",
                Status::Error,
                "Configured executable is unavailable; install it or fix its path in ai config.",
            );
            return Ok(report);
        }
    };
    report.add("binary", Status::Ok, "Configured executable is available.");
    process::install_signal_handler()?;
    let version = probe(&binary, &["--version"], timeout)?;
    let version_ok = version.is_some_and(|o| o.status.success());
    report.add(
        "version",
        if version_ok {
            Status::Ok
        } else {
            Status::Error
        },
        if version_ok {
            "Executable responds to --version."
        } else {
            "Version probe failed, timed out or exceeded 64 KiB; check the native CLI locally."
        },
    );
    let args: &[&str] = match selected.provider {
        Kind::Claude => &["auth", "status"],
        Kind::Openai => &["login", "status"],
        Kind::Ollama => &["list"],
        _ => {
            report.add(
                "authentication",
                Status::Warning,
                "Not checked for this provider; verify authentication in its native CLI.",
            );
            return Ok(report);
        }
    };
    let output = probe(&binary, args, timeout)?;
    let success = output.is_some_and(|o| {
        o.status.success()
            && (selected.provider != Kind::Claude
                || serde_json::from_slice::<serde_json::Value>(&o.stdout)
                    .ok()
                    .and_then(|v| v.get("loggedIn").and_then(|v| v.as_bool()))
                    == Some(true))
    });
    let (name, good, bad) = match selected.provider {
        Kind::Ollama => (
            "service",
            "Ollama responds to list; model availability is not verified.",
            "Ollama list failed; start the local service and check its configuration.",
        ),
        Kind::Claude => (
            "authentication",
            "Native CLI reports logged in; model access and quota are not verified.",
            "Authentication status failed; run claude auth login in your terminal.",
        ),
        _ => (
            "authentication",
            "Native CLI reports logged in; model access and quota are not verified.",
            "Authentication status failed; run codex login in your terminal.",
        ),
    };
    report.add(
        name,
        if success { Status::Ok } else { Status::Error },
        if success { good } else { bad },
    );
    Ok(report)
}
