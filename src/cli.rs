use clap::{Parser, Subcommand, error::ErrorKind};
use std::{ffi::OsString, path::PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "ai",
    version,
    about = "Run a chosen AI provider with project context",
    after_help = "Usage: ai [--profile NAME] [provider] [model-code] [flags] <message>\nSet defaults: ai setup cl hk4.5\nProfiles: ai profile create 2 cl hk4.5 --instructions 'Review carefully' --install\nUse profile: ai2 review this code (or ai --profile 2 for scripts)\nProviders: cl/claude, cp/copilot, ag/antigravity, oa/codex/openai, ol/ollama (or a configured alias).\nShort form: ai cl hk4.5 explain this function\nList shorthand models: ai models cl\nRun ai <provider> --help for request options."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Admin>,
}

/// Parsed separately so request options come after the chosen provider.
#[derive(Parser, Debug)]
#[command(
    name = "ai <provider>",
    args_override_self = true,
    about = "Send a single request to the chosen provider"
)]
pub struct Request {
    /// Override the configured model with a full model ID or shorthand (e.g. hk4.5).
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
    /// Print the assembled prompt without invoking the provider.
    #[arg(long)]
    pub dry_run: bool,
    /// List assembled prompt sources and byte sizes without invoking a provider.
    #[arg(long)]
    pub explain: bool,
    /// Skip Git subprocesses.
    #[arg(long)]
    pub no_git: bool,
    /// Ignore project context for this request.
    #[arg(long)]
    pub no_context: bool,
    /// Maximum provider execution time in seconds (including output delivery).
    #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=86400))]
    pub timeout: u64,
    /// Combined stdout/stderr byte limit; applies to streamed and buffered output.
    #[arg(long, default_value_t = 16_777_216, value_parser = clap::value_parser!(u64).range(1..=1_073_741_824))]
    pub max_output_bytes: u64,
    /// Buffer the response until success instead of forwarding text as it arrives.
    #[arg(long)]
    pub no_stream: bool,
    /// Forward one native argument. Repeat for values: --provider-arg=--effort --provider-arg=high.
    #[arg(long, value_name = "ARG", allow_hyphen_values = true)]
    pub provider_arg: Vec<OsString>,
    /// Attach file contents directly to context. Repeatable.
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub file: Vec<PathBuf>,
    /// Include working directory git diff in context.
    #[arg(long)]
    pub diff: bool,
    /// Include staged git diff in context.
    #[arg(long)]
    pub staged: bool,
    /// Copy provider output to system clipboard.
    #[arg(short = 'c', long = "copy")]
    pub copy: bool,
    /// Message words; flags after the first word are literal text. Use -- for leading hyphens.
    #[arg(trailing_var_arg = true)]
    pub message: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Admin {
    /// Save and run reusable instructions and request flags.
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Set the default provider and optionally its model; without arguments, show defaults.
    Setup {
        provider: Option<String>,
        model: Option<String>,
    },
    /// Manage independent provider/model defaults and agent instructions.
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
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
    /// Check configuration, executable and native authentication without a model request.
    Doctor {
        provider: Option<String>,
        #[arg(long)]
        json: bool,
        /// Deadline per diagnostic subprocess.
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=30))]
        timeout: u64,
    },
    /// List configured shorthand mappings (no network calls).
    Models {
        provider: Option<String>,
    },
    /// Generate shell completion scripts for bash, zsh, fish, powershell, or elvish.
    Completions {
        shell: clap_complete::Shell,
    },
    #[command(external_subcommand)]
    Provider(Vec<String>),
}
#[derive(Subcommand, Debug)]
pub enum TaskAction {
    /// Create or replace a task. Saved file paths resolve from the invocation directory.
    Set {
        name: String,
        #[arg(long)]
        instructions: String,
        #[arg(long)]
        profile: Option<String>,
        /// Save one request argument; repeat for values, e.g. --arg=--timeout --arg=60.
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
    },
    List,
    Show {
        name: String,
    },
    Remove {
        name: String,
    },
    #[command(external_subcommand)]
    Run(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum ProfileAction {
    /// Update a profile; omitted fields are preserved. Changing provider resets its model.
    #[command(alias = "create")]
    Set {
        name: String,
        provider: Option<String>,
        model: Option<String>,
        /// Agent instructions prepended to requests. Pass an empty string to clear.
        #[arg(long, conflicts_with = "instructions_file")]
        instructions: Option<String>,
        /// Link a UTF-8 instructions file, read afresh on each request.
        #[arg(long, value_name = "PATH")]
        instructions_file: Option<PathBuf>,
        /// Install ai<N> (or ai-<name>) beside this executable, or in --bin-dir.
        #[arg(long)]
        install: bool,
        #[arg(long, requires = "install", value_name = "DIR")]
        bin_dir: Option<PathBuf>,
    },
    /// Export resolved defaults and embedded instructions (review before sharing).
    Export {
        name: String,
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Import a portable profile without installing shortcuts or changing global defaults.
    Import {
        path: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        replace: bool,
    },
    /// Show all saved profiles including their instructions.
    List,
    Remove {
        name: String,
    },
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
    /// Show included sources and exact assembled prompt bytes without calling a provider.
    Explain {
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        request: Request,
    },
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
    if let Some(first) = args.first().filter(|s| !s.starts_with('-'))
        && config.resolve(first).is_ok()
    {
        return Ok((first.clone(), Request::try_from_provider_args(args)?));
    }
    let argv = std::iter::once("ai".to_owned()).chain(args.iter().cloned());
    Ok((
        config.default_provider.clone(),
        Request::try_parse_from(argv)?,
    ))
}
