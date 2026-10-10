use ainline::{
    cli::{Admin, Cli, ConfigAction, ContextAction, Request, select_request},
    config::{self, Config},
    context,
    provider::{self, ProviderError},
    storage,
};
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use std::io::{self, IsTerminal, Read, Write};

fn main() {
    if let Err(error) = run() {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
        {
            return;
        }
        eprintln!("ai: {error:#}");
        let code = match error.downcast_ref::<ProviderError>() {
            Some(ProviderError::Interrupted) => 130,
            Some(ProviderError::Failed {
                code: Some(code), ..
            }) => *code,
            _ => 1,
        };
        std::process::exit(code);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let administrative = args.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "setup"
                | "config"
                | "context"
                | "status"
                | "models"
                | "help"
                | "--help"
                | "-h"
                | "--version"
                | "-V"
        )
    });
    if !administrative {
        if args.is_empty() && io::stdin().is_terminal() {
            Cli::command().print_help()?;
            println!();
            return Ok(());
        }
        let config = Config::load()?;
        let (id, options) = select_request(&config, &args).unwrap_or_else(|e| e.exit());
        return request(&config, &id, options, args.is_empty());
    }
    let Some(command) = Cli::parse().command else {
        return Ok(());
    };
    let cwd = std::env::current_dir()?;
    let root = context::finder::find(&cwd);
    match command {
        Admin::Setup { provider, model } => {
            if let Some(provider) = provider {
                Config::set_defaults(&provider, model.as_deref(), true)?;
            }
            let config = Config::load()?;
            let selected = config.resolve(&config.default_provider)?;
            println!(
                "Default provider: {}\nDefault model: {}\nConfig: {}",
                config.default_provider,
                selected.model,
                config::path()?.display()
            );
            println!("Set defaults: ai setup <provider> [model]");
        }
        Admin::Config { action } => match action {
            ConfigAction::Model { provider, model } => {
                println!(
                    "{}",
                    Config::set_defaults(&provider, Some(&model), false)?.display()
                );
            }
            ConfigAction::Init => println!("{}", Config::init()?.display()),
            ConfigAction::Path => println!("{}", config::path()?.display()),
            ConfigAction::Show => print!("{}", serde_yaml_ng::to_string(&Config::load()?)?),
            ConfigAction::Validate => {
                Config::load()?;
                println!("Configuration valid");
            }
        },
        Admin::Context { action } => match action {
            ContextAction::Init => {
                context::init(&root)?;
                println!("{}", root.join(".ai").display());
            }
            ContextAction::Show => print!("{}", context::collect(&root, true)?),
            ContextAction::Note { text } => context::session::Session::add(&root, text.join(" "))?,
            ContextAction::Clear => context::session::Session::clear(&root)?,
            ContextAction::Rule { name, text, local } => {
                context::rules::add(&root, &name, &text.join(" "), local)?
            }
        },
        Admin::Models { provider } => {
            let config = Config::load()?;
            let selected = provider
                .as_deref()
                .map(|id| config.resolve(id))
                .transpose()?;
            for (kind, entries) in &config.model_aliases {
                if selected
                    .as_ref()
                    .is_some_and(|model| model.provider != *kind)
                {
                    continue;
                }
                for (code, model) in entries {
                    println!("{kind:?}\t{code}\t{model}");
                }
            }
        }
        Admin::Status => {
            let config = Config::load()?;
            println!(
                "Project: {}\nConfig: {}\nDefault provider: {}",
                root.display(),
                config::path()?.display(),
                config.default_provider
            );
            for (id, model) in &config.models {
                println!(
                    "{id}: {} | model: {} | binary: {}",
                    if which::which(&model.binary).is_ok() {
                        "available"
                    } else {
                        "missing"
                    },
                    model.model,
                    model.binary
                );
            }
        }
        Admin::Provider(_) => unreachable!("handled before filesystem work"),
    }
    Ok(())
}

fn request(config: &Config, id: &str, options: Request, help_if_empty: bool) -> Result<()> {
    let mut model = config.resolve(id)?;
    if let Some(name) = options.model {
        model.model = config.resolve_model(model.provider, &name)?;
    }
    provider::validate_args(&options.provider_arg)?;
    let mut prompt = options.message.join(" ");
    if !io::stdin().is_terminal() {
        let mut piped = String::new();
        io::stdin()
            .take(storage::MAX_FILE + 1)
            .read_to_string(&mut piped)?;
        anyhow::ensure!(
            piped.len() as u64 <= storage::MAX_FILE,
            "stdin exceeds 1 MiB"
        );
        if !piped.is_empty() {
            if !prompt.is_empty() {
                prompt.push_str("\n\nInput:\n");
            }
            prompt.push_str(&piped);
        }
    }
    if help_if_empty && prompt.trim().is_empty() {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    }
    anyhow::ensure!(
        !prompt.trim().is_empty(),
        "provide a message or pipe input: ai {id} [flags] <message>"
    );
    let cwd = std::env::current_dir()?;
    let request = if options.no_context {
        prompt
    } else {
        let root = context::finder::find(&cwd);
        format!(
            "Project context:\n{}\nUser request:\n{prompt}",
            context::collect(&root, !options.no_git)?
        )
    };
    if options.dry_run {
        println!("{request}");
        return Ok(());
    }
    let binary = which::which(&model.binary)
        .with_context(|| format!("provider '{id}' binary '{}' not found", model.binary))?;
    let output =
        provider::execute_with_args(&model, &binary, &request, &cwd, &options.provider_arg)
            .with_context(|| format!("provider '{id}' failed"))?;
    io::stdout().lock().write_all(&output)?;
    Ok(())
}
