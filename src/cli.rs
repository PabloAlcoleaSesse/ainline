use clap::{Parser, Subcommand, error::ErrorKind};
use std::ffi::OsString;

#[derive(Parser, Debug)]
#[command(
    name = "ai",
    version,
    about = "Run a chosen AI provider with project context",
    after_help = "Usage: ai [provider] [model-code] [flags] <message>\nSet defaults: ai setup cl hk4.5\nProviders: cl/claude, cp/copilot, ag/antigravity, oa/openai, ol/ollama (or a configured alias).\nShort form: ai cl hk4.5 explain this function\nList shorthand models: ai models cl\nRun ai <provider> --help for request options."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Admin>,
}

/// Parsed separately so request options come after the chosen provider.
#[derive(Parser, Debug)]
#[command(
    name = "ai <provider>",
    about = "Send a single request to the chosen provider"
)]
pub struct Request {
    /// Override the configured model with a full model ID or shorthand (e.g. hk4.5).
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
    /// Print the assembled prompt without invoking the provider.
    #[arg(long)]
    pub dry_run: bool,
    /// Skip Git subprocesses.
    #[arg(long)]
    pub no_git: bool,
    /// Ignore project context for this request.
    #[arg(long)]
    pub no_context: bool,
    /// Forward one native argument. Repeat for values: --provider-arg=--effort --provider-arg=high.
    #[arg(long, value_name = "ARG", allow_hyphen_values = true)]
    pub provider_arg: Vec<OsString>,
    /// Message words; flags after the first word are literal text. Use -- for leading hyphens.
    #[arg(trailing_var_arg = true)]
    pub message: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Admin {
    /// Set the default provider and optionally its model; without arguments, show defaults.
    Setup {
        provider: Option<String>,
        model: Option<String>,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },
    Status,
    /// List configured shorthand mappings (no network calls).
    Models {
        provider: Option<String>,
    },
    #[command(external_subcommand)]
    Provider(Vec<String>),
}
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Set one provider's default model without changing the default provider.
    Model {
        provider: String,
        model: String,
    },
    Init,
    Show,
    Path,
    Validate,
}
#[derive(Subcommand, Debug)]
pub enum ContextAction {
    Init,
    Show,
    Note {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    Clear,
    Rule {
        name: String,
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
        #[arg(long)]
        local: bool,
    },
}

impl Request {
    pub fn try_from_provider_args(args: &[String]) -> Result<Self, clap::Error> {
        let Some(provider) = args.first() else {
            return Err(clap::Error::raw(
                ErrorKind::MissingRequiredArgument,
                "provider is required",
            ));
        };
        let short = args.get(1).filter(|arg| crate::aliases::is_model_code(arg));
        let skip = if short.is_some() { 2 } else { 1 };
        let argv = std::iter::once(format!("ai {provider}")).chain(args.iter().skip(skip).cloned());
        let mut request = Self::try_parse_from(argv)?;
        if let Some(short) = short {
            if request.model.is_some() {
                return Err(clap::Error::raw(
                    ErrorKind::ArgumentConflict,
                    "choose either a positional model shorthand or --model, not both",
                ));
            }
            request.model = Some(short.clone());
        }
        Ok(request)
    }
}

/// Only recognized provider names select a provider; otherwise all words are a message.
/// Leading flags or -- always use the configured default and bypass provider detection.
pub fn select_request(
    config: &crate::config::Config,
    args: &[String],
) -> Result<(String, Request), clap::Error> {
    if let Some(first) = args.first().filter(|s| !s.starts_with('-')) {
        if config.resolve(first).is_ok() {
            return Ok((first.clone(), Request::try_from_provider_args(args)?));
        }
    }
    let argv = std::iter::once("ai".to_owned()).chain(args.iter().cloned());
    Ok((
        config.default_provider.clone(),
        Request::try_parse_from(argv)?,
    ))
}
