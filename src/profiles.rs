use crate::{
    config::{self, Config},
    storage,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_file: Option<PathBuf>,
}

pub fn validate_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
        "profile names must be 1–64 ASCII letters, digits, hyphens or underscores"
    );
    Ok(())
}

/// A symlink or renamed executable ai<N> selects numeric profile <N>.
pub fn invoked_profile(executable: &str) -> Option<String> {
    let name = Path::new(executable).file_name()?.to_str()?;
    let name = name.strip_suffix(".exe").unwrap_or(name);
    if let Some(name) = name.strip_prefix("ai-") {
        return validate_name(name).ok().map(|()| name.to_owned());
    }
    let suffix = name.strip_prefix("ai")?;
    (!suffix.is_empty() && suffix.bytes().all(|c| c.is_ascii_digit())).then(|| suffix.to_owned())
}

/// Update only supplied fields. An empty instructions string explicitly clears instructions.
pub fn set(
    name: &str,
    provider: Option<String>,
    model: Option<String>,
    instructions: Option<String>,
) -> Result<()> {
    set_with_file(name, provider, model, instructions, None)
}

pub fn set_with_file(
    name: &str,
    provider: Option<String>,
    model: Option<String>,
    instructions: Option<String>,
    instructions_file: Option<PathBuf>,
) -> Result<()> {
    validate_name(name)?;
    anyhow::ensure!(
        instructions.is_none() || instructions_file.is_none(),
        "choose inline instructions or an instructions file"
    );
    let instructions_file = instructions_file
        .map(|p| {
            p.canonicalize()
                .with_context(|| format!("locating instructions file {}", p.display()))
        })
        .transpose()?;
    if let Some(path) = &instructions_file {
        anyhow::ensure!(path.is_file(), "instructions path must be a regular file");
        storage::read(path)?.context("instructions file is missing")?;
    }
    let path = config::path()?;
    let _lock = storage::lock(&path.with_extension("lock"))?;
    let mut config = Config::load()?;
    let profile = config.profiles.entry(name.into()).or_default();
    if let Some(provider) = provider {
        if profile.provider.as_ref() != Some(&provider) {
            profile.model = None;
        }
        profile.provider = Some(provider);
    }
    if model.is_some() {
        profile.model = model;
    }
    if let Some(instructions) = instructions {
        profile.instructions = instructions;
        profile.instructions_file = None;
    }
    if let Some(path) = instructions_file {
        profile.instructions.clear();
        profile.instructions_file = Some(path);
    }
    config.validate()?;
    let serialized = serde_yaml_ng::to_string(&config)?;
    anyhow::ensure!(
        serialized.len() as u64 <= storage::MAX_FILE,
        "config exceeds 1 MiB"
    );
    storage::write(&path, serialized.as_bytes())
}

pub fn remove(name: &str) -> Result<()> {
    let path = config::path()?;
    let _lock = storage::lock(&path.with_extension("lock"))?;
    let mut config = Config::load()?;
    config
        .profiles
        .remove(name)
        .with_context(|| format!("unknown profile '{name}'"))?;
    storage::write(&path, serde_yaml_ng::to_string(&config)?.as_bytes())
}

impl Profile {
    pub fn instructions_text(&self) -> Result<String> {
        match &self.instructions_file {
            Some(path) => storage::read(path)?
                .with_context(|| format!("instructions file missing: {}", path.display())),
            None => Ok(self.instructions.clone()),
        }
    }
}

/// Install without overwriting any existing command. No shell startup files are changed.
pub fn install(name: &str, bin_dir: Option<&Path>) -> Result<PathBuf> {
    validate_name(name)?;
    let executable = std::env::current_exe()?;
    #[cfg(unix)]
    let executable = executable.canonicalize()?;
    let dir = bin_dir.unwrap_or(executable.parent().context("executable has no parent")?);
    std::fs::create_dir_all(dir)?;
    let command = if name.bytes().all(|c| c.is_ascii_digit()) {
        format!("ai{name}")
    } else {
        format!("ai-{name}")
    };
    #[cfg(unix)]
    {
        let path = dir.join(command);
        if std::fs::read_link(&path).is_ok_and(|target| target == executable) {
            return Ok(path);
        }
        std::os::unix::fs::symlink(&executable, &path).with_context(|| {
            format!(
                "profile saved, but could not install {}; existing commands are never overwritten",
                path.display()
            )
        })?;
        Ok(path)
    }
    #[cfg(windows)]
    {
        use std::io::Write;
        let path = dir.join(format!("{command}.cmd"));
        let exe = executable
            .to_str()
            .context("launcher needs a Unicode executable path")?;
        anyhow::ensure!(
            !exe.contains(['%', '"', '\r', '\n']),
            "unsupported character in executable path"
        );
        let content = format!(
            "@echo off\r\nsetlocal DisableDelayedExpansion\r\n\"{exe}\" --profile={name} %*\r\nexit /b %errorlevel%\r\n"
        );
        if std::fs::read_to_string(&path).is_ok_and(|s| s == content) {
            return Ok(path);
        }
        let mut file = std::fs::OpenOptions::new().create_new(true).write(true).open(&path)
            .with_context(|| format!("profile saved, but could not install {}; existing commands are never overwritten", path.display()))?;
        file.write_all(content.as_bytes())?;
        Ok(path)
    }
}

/// Portable data only: never imports executable paths or linked local files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    version: u32,
    provider: config::Kind,
    model: String,
    instructions: String,
}

pub fn export(name: &str, output: Option<&Path>) -> Result<String> {
    use std::io::Write;
    let config = Config::load()?;
    let profile = config.profiles.get(name).context("unknown profile")?;
    let selected = config.resolve(
        profile
            .provider
            .as_deref()
            .unwrap_or(&config.default_provider),
    )?;
    let model = match &profile.model {
        Some(model) => config.resolve_model(selected.provider, model)?,
        None => selected.model,
    };
    if let Some(path) = &profile.instructions_file {
        anyhow::ensure!(path.is_file(), "instructions must be a regular file");
    }
    let text = serde_yaml_ng::to_string(&Bundle {
        version: 1,
        provider: selected.provider,
        model,
        instructions: profile.instructions_text()?,
    })?;
    anyhow::ensure!(
        text.len() as u64 <= storage::MAX_FILE,
        "export exceeds 1 MiB"
    );
    if let Some(path) = output {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(text.as_bytes())?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(path)
            .context("export destination must not already exist")?;
    }
    Ok(text)
}

pub fn import(source: &Path, name: &str, replace: bool) -> Result<()> {
    validate_name(name)?;
    anyhow::ensure!(source.is_file(), "import must be a regular file");
    let bundle: Bundle =
        serde_yaml_ng::from_str(&storage::read(source)?.context("missing import file")?)?;
    anyhow::ensure!(bundle.version == 1, "unsupported profile format version");
    let id = match bundle.provider {
        config::Kind::Claude => "claude",
        config::Kind::Openai => "openai",
        config::Kind::Ollama => "ollama",
        config::Kind::Copilot => "copilot",
        config::Kind::Antigravity => "antigravity",
    };
    let path = config::path()?;
    let _lock = storage::lock(&path.with_extension("lock"))?;
    let mut config = Config::load()?;
    anyhow::ensure!(
        replace || !config.profiles.contains_key(name),
        "profile already exists; use --replace to overwrite it"
    );
    anyhow::ensure!(
        config.resolve(id)?.provider == bundle.provider,
        "local canonical provider has a different kind"
    );
    // A portable model must retain its meaning under the recipient's aliases.
    anyhow::ensure!(
        config.resolve_model(bundle.provider, &bundle.model)? == bundle.model,
        "imported model conflicts with a local shorthand"
    );
    config.profiles.insert(
        name.into(),
        Profile {
            provider: Some(id.into()),
            model: Some(bundle.model),
            instructions: bundle.instructions,
            instructions_file: None,
        },
    );
    config.validate()?;
    let text = serde_yaml_ng::to_string(&config)?;
    anyhow::ensure!(
        text.len() as u64 <= storage::MAX_FILE,
        "config exceeds 1 MiB"
    );
    storage::write(&path, text.as_bytes())
}
