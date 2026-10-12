use crate::{
    config::{self, Config},
    context, provider,
};
use anyhow::{Context, Result};
use std::{fmt::Write, path::Path};

/// Inspect the same defaults and instructions as a request, without executing a provider.
pub fn report(config: &Config, name: Option<&str>, root: &Path) -> Result<String> {
    let profile = name
        .map(|name| {
            config
                .profiles
                .get(name)
                .with_context(|| format!("unknown profile '{name}'"))
        })
        .transpose()?;
    let id = profile
        .and_then(|p| p.provider.as_deref())
        .unwrap_or(&config.default_provider);
    let mut model = config.resolve(id)?;
    if let Some(name) = profile.and_then(|p| p.model.as_deref()) {
        model.model = config.resolve_model(model.provider, name)?;
    }
    let mut text = format!(
        "Project: {}\nConfig: {}\nProfile: {}\nEffective provider: {id}\nEffective model: {}\nBinary: {} ({})\n",
        root.display(),
        config::path()?.display(),
        name.unwrap_or("none (global defaults)"),
        model.model,
        model.binary,
        if which::which(&model.binary).is_ok() {
            "available"
        } else {
            "missing"
        }
    );
    writeln!(
        text,
        "Output: {}",
        if provider::supports_streaming(model.provider) {
            "streaming text (Claude deltas; native text for Copilot/Ollama)"
        } else {
            if model.provider == crate::config::Kind::Openai {
                "Codex final answer buffered until successful exit"
            } else {
                "structured response buffered and validated"
            }
        }
    )?;
    writeln!(
        text,
        "Limits: 300 seconds; 16777216 output bytes (override with request flags)"
    )?;
    if let Some(profile) = profile {
        writeln!(
            text,
            "Instructions source: {}",
            profile
                .instructions_file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "inline profile text".into())
        )?;
        writeln!(text, "Instructions:\n{}", profile.instructions_text()?)?;
    } else {
        writeln!(text, "Instructions: none")?;
    }
    writeln!(text, "Context sources:")?;
    for (name, _) in context::rules::load(root)? {
        writeln!(
            text,
            "  rule: {}",
            root.join(".ai/rules").join(name).display()
        )?;
    }
    let session = context::session::Session::load(root)?;
    writeln!(
        text,
        "  session: {} ({} live notes)",
        root.join(".ai/session.json").display(),
        session.notes.len()
    )?;
    writeln!(
        text,
        "  environment: {}",
        context::env::collect(root, false).trim()
    )?;
    writeln!(
        text,
        "  Git: branch and diff stat collected on requests unless --no-git"
    )?;
    writeln!(text, "Available configuration entries:")?;
    for (id, model) in &config.models {
        writeln!(
            text,
            "  {id}: {} | model: {} | binary: {}",
            if which::which(&model.binary).is_ok() {
                "available"
            } else {
                "missing"
            },
            model.model,
            model.binary
        )?;
    }
    Ok(text)
}
