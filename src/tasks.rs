//! Global reusable prompts. Task data never executes shell commands.
use crate::{cli::Request, config, profiles, storage};
use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub instructions: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
}
fn path() -> Result<std::path::PathBuf> {
    Ok(config::path()?.with_file_name("tasks.yaml"))
}
pub fn load() -> Result<BTreeMap<String, Task>> {
    let tasks: BTreeMap<String, Task> = storage::read(&path()?)?
        .map(|s| serde_yaml_ng::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    for (name, task) in &tasks {
        validate(name, task)?;
    }
    Ok(tasks)
}
fn validate(name: &str, task: &Task) -> Result<()> {
    profiles::validate_name(name)?;
    anyhow::ensure!(
        !["set", "list", "show", "remove", "help"].contains(&name),
        "reserved task name"
    );
    anyhow::ensure!(
        !task.instructions.trim().is_empty(),
        "task instructions cannot be empty"
    );
    if let Some(profile) = &task.profile {
        profiles::validate_name(profile)?;
    }
    let request =
        Request::try_parse_from(std::iter::once("ai").chain(task.args.iter().map(String::as_str)))?;
    anyhow::ensure!(
        request.message.is_empty(),
        "saved task args must contain flags only; use instructions for the prompt"
    );
    crate::provider::validate_args(&request.provider_arg)?;
    Ok(())
}
pub fn set(name: &str, task: Task) -> Result<()> {
    validate(name, &task)?;
    if let Some(profile) = &task.profile {
        anyhow::ensure!(
            config::Config::load()?.profiles.contains_key(profile),
            "unknown task profile"
        );
    }
    let path = path()?;
    let _lock = storage::lock(&path.with_extension("lock"))?;
    let mut tasks = load()?;
    tasks.insert(name.into(), task);
    let text = serde_yaml_ng::to_string(&tasks)?;
    anyhow::ensure!(text.len() as u64 <= storage::MAX_FILE, "tasks exceed 1 MiB");
    storage::write(&path, text.as_bytes())
}
pub fn remove(name: &str) -> Result<()> {
    let path = path()?;
    let _lock = storage::lock(&path.with_extension("lock"))?;
    let mut tasks = load()?;
    tasks.remove(name).context("unknown task")?;
    storage::write(&path, serde_yaml_ng::to_string(&tasks)?.as_bytes())
}
