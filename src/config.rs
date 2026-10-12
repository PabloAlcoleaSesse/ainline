use crate::{
    aliases::{self, ModelAliases},
    storage,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Antigravity,
    Copilot,
    Claude,
    Ollama,
    #[serde(alias = "codex")]
    Openai,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub provider: Kind,
    pub model: String,
    pub binary: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub models: BTreeMap<String, Model>,
    #[serde(default)]
    pub model_aliases: ModelAliases,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, crate::profiles::Profile>,
}
fn default_provider() -> String {
    "claude".into()
}

impl Default for Config {
    fn default() -> Self {
        let models = [
            ("claude", Kind::Claude, "sonnet", "claude"),
            ("openai", Kind::Openai, "default", "codex"),
            ("copilot", Kind::Copilot, "claude-haiku-4.5", "copilot"),
            (
                "antigravity",
                Kind::Antigravity,
                "gemini-3.1-pro-high",
                "agy",
            ),
            ("ollama", Kind::Ollama, "qwen2.5-coder:7b", "ollama"),
        ]
        .into_iter()
        .map(|(id, provider, model, binary)| {
            (
                id.into(),
                Model {
                    provider,
                    model: model.into(),
                    binary: binary.into(),
                },
            )
        })
        .collect();
        Self {
            default_provider: default_provider(),
            models,
            model_aliases: aliases::defaults(),
            profiles: BTreeMap::new(),
        }
    }
}
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("AI_CONFIG_FILE") {
        return Ok(PathBuf::from(p));
    }
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("cannot locate home directory")?;
    Ok(root.join("ai/config.yaml"))
}
impl Config {
    pub fn load() -> Result<Self> {
        let p = path()?;
        let config = match storage::read(&p)? {
            Some(s) => {
                Self::parse(&s).with_context(|| format!("invalid config {}", p.display()))?
            }
            None => Self::default(),
        };
        config.validate()?;
        Ok(config)
    }
    /// Accept the old routing config without reading or writing a usage ledger.
    /// Remove only retired fields; unknown fields still fail validation.
    pub fn parse(text: &str) -> Result<Self> {
        let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)?;
        if let Some(root) = value.as_mapping_mut() {
            root.remove(serde_yaml_ng::Value::from("priorities"));
            if let Some(models) = root
                .get_mut(serde_yaml_ng::Value::from("models"))
                .and_then(|v| v.as_mapping_mut())
            {
                for model in models.values_mut().filter_map(|v| v.as_mapping_mut()) {
                    for key in ["daily_requests", "daily_budget_usd", "estimated_cost_usd"] {
                        model.remove(serde_yaml_ng::Value::from(key));
                    }
                }
            }
        }
        let mut config: Self = serde_yaml_ng::from_value(value)?;
        // Migrate the former API CLI's generated executable/default model. Preserve
        // custom executable paths and explicit alternative model choices.
        for model in config
            .models
            .values_mut()
            .filter(|m| m.provider == Kind::Openai && m.binary == "openai")
        {
            model.binary = "codex".into();
            if model.model == "gpt-4.1-mini" {
                model.model = "default".into();
            }
        }
        // Built-in names remain available when a config contains only custom aliases.
        for (id, model) in Self::default().models {
            config.models.entry(id).or_insert(model);
        }
        for (kind, entries) in aliases::defaults() {
            let configured = config.model_aliases.entry(kind).or_default();
            for (code, model) in entries {
                configured.entry(code).or_insert(model);
            }
        }
        config.validate()?;
        Ok(config)
    }

    pub fn resolve(&self, id: &str) -> Result<Model> {
        // Existing exact config aliases take precedence over shorthand expansion.
        let id = if self.models.contains_key(id) {
            id
        } else {
            aliases::provider(id)
        };
        self.models.get(id).cloned().with_context(|| format!(
            "unknown provider '{id}'; choose cl/claude, cp/copilot, ag/antigravity, oa/codex/openai, ol/ollama or a configured alias"
        ))
    }

    pub fn resolve_model(&self, kind: Kind, name: &str) -> Result<String> {
        anyhow::ensure!(!name.trim().is_empty(), "model cannot be empty");
        if let Some(model) = self
            .model_aliases
            .get(&kind)
            .and_then(|entries| entries.get(name))
        {
            return Ok(model.clone());
        }
        anyhow::ensure!(
            !aliases::is_model_code(name),
            "unknown model shorthand '{name}' for {kind:?}; run ai models <provider>, use --model with a full ID, or use -- before literal message text"
        );
        Ok(name.to_owned())
    }

    pub fn validate(&self) -> Result<()> {
        self.resolve(&self.default_provider)
            .context("invalid default provider")?;
        for (id, m) in &self.models {
            anyhow::ensure!(
                !id.is_empty() && !m.model.trim().is_empty() && !m.binary.trim().is_empty(),
                "empty model or binary: {id}"
            );
            anyhow::ensure!(
                ![
                    "config",
                    "context",
                    "status",
                    "models",
                    "setup",
                    "profile",
                    "help",
                    "doctor",
                    "task",
                    "completions"
                ]
                .contains(&id.as_str())
                    && !id.starts_with('-')
                    && !id.chars().any(char::is_whitespace),
                "invalid or reserved provider alias: {id}"
            );
        }
        for entries in self.model_aliases.values() {
            for (code, model) in entries {
                anyhow::ensure!(
                    aliases::is_model_code(code),
                    "model shorthand '{code}' must be two lowercase letters followed by a numeric version"
                );
                anyhow::ensure!(
                    !model.trim().is_empty()
                        && !model.starts_with('-')
                        && !model.chars().any(char::is_whitespace),
                    "invalid model ID for shorthand '{code}'"
                );
            }
        }
        for (name, profile) in &self.profiles {
            crate::profiles::validate_name(name)?;
            if let Some(path) = &profile.instructions_file {
                anyhow::ensure!(
                    path.is_absolute(),
                    "profile '{name}' instructions_file must be absolute"
                );
                anyhow::ensure!(
                    profile.instructions.is_empty(),
                    "profile '{name}' cannot combine inline and file instructions"
                );
            }
            let model = self.resolve(
                profile
                    .provider
                    .as_deref()
                    .unwrap_or(&self.default_provider),
            )?;
            if let Some(name) = &profile.model {
                self.resolve_model(model.provider, name)?;
            }
        }
        Ok(())
    }
    /// Update defaults atomically while preserving other providers and context settings.
    pub fn set_defaults(
        provider: &str,
        model: Option<&str>,
        make_default: bool,
    ) -> Result<PathBuf> {
        let path = path()?;
        let _lock = storage::lock(&path.with_extension("lock"))?;
        let mut config = Self::load()?;
        let selected = config.resolve(provider)?;
        let key = if config.models.contains_key(provider) {
            provider
        } else {
            aliases::provider(provider)
        };
        if let Some(model) = model {
            let resolved = config.resolve_model(selected.provider, model)?;
            config
                .models
                .get_mut(key)
                .context("missing provider entry")?
                .model = resolved;
        }
        if make_default {
            config.default_provider = key.to_owned();
        }
        config.validate()?;
        storage::write(&path, serde_yaml_ng::to_string(&config)?.as_bytes())?;
        Ok(path)
    }

    pub fn init() -> Result<PathBuf> {
        let p = path()?;
        let _lock = storage::lock(&p.with_extension("lock"))?;
        anyhow::ensure!(!p.exists(), "config already exists: {}", p.display());
        storage::write(&p, serde_yaml_ng::to_string(&Self::default())?.as_bytes())?;
        Ok(p)
    }
}
