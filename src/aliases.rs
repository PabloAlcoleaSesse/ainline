//! Explicit local aliases: no network lookup and no inferred future model IDs.
use crate::config::Kind;
use std::collections::BTreeMap;

pub type ModelAliases = BTreeMap<Kind, BTreeMap<String, String>>;

pub fn provider(name: &str) -> &str {
    match name {
        "cl" => "claude",
        "cp" => "copilot",
        "ag" => "antigravity",
        "oa" => "openai",
        "ol" | "local" => "ollama",
        _ => name,
    }
}

/// Two lowercase ASCII letters followed by a numeric version, e.g. hk4.5 or qw2.
pub fn is_model_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[..2].iter().all(u8::is_ascii_lowercase)
        && value[2..]
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

pub fn defaults() -> ModelAliases {
    [
        (Kind::Claude, "hk4.5", "claude-haiku-4-5-20251001"),
        (Kind::Claude, "sn4.5", "claude-sonnet-4-5-20250929"),
        (Kind::Claude, "op4.5", "claude-opus-4-5-20251101"),
        (Kind::Copilot, "hk4.5", "claude-haiku-4.5"),
        (Kind::Antigravity, "gp3.1", "gemini-3.1-pro-high"),
        (Kind::Openai, "gm4.1", "gpt-4.1-mini"),
        (Kind::Ollama, "qw2.5", "qwen2.5-coder:7b"),
    ]
    .into_iter()
    .fold(BTreeMap::new(), |mut aliases, (provider, code, model)| {
        aliases
            .entry(provider)
            .or_default()
            .insert(code.into(), model.into());
        aliases
    })
}
