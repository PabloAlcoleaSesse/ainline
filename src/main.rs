use ainline::{
    cli::{
        Admin, Cli, ConfigAction, ContextAction, ProfileAction, Request, TaskAction, select_request,
    },
    config::{self, Config},
    context, doctor,
    profiles::{self, Profile},
    prompt,
    provider::{self, ProviderError},
    status, storage, tasks,
};
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use std::io::{self, IsTerminal, Read};

fn main() {
    if let Err(error) = run() {
        if error.chain().any(|cause| {
            cause
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
        }) {
            return;
        }
        if error.is::<doctor::Unhealthy>() {
            std::process::exit(1);
        }
        provider::process::report_error(format!("ai: {error:#}\n"));
        let code = match error.downcast_ref::<ProviderError>() {
            Some(ProviderError::Interrupted) => 130,
            Some(ProviderError::Timeout) => 124,
            Some(ProviderError::Failed {
                code: Some(code), ..
            }) => *code,
            _ => 1,
        };
        std::process::exit(code);
    }
}

fn run() -> Result<()> {
    let executable = std::env::args().next().unwrap_or_default();
    let mut profile_name = profiles::invoked_profile(&executable);
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--profile") {
        if args.get(1).is_none_or(|name| name.starts_with('-')) {
            clap::Error::raw(
                clap::error::ErrorKind::MissingRequiredArgument,
                "--profile requires a name",
            )
            .exit();
        }
        profile_name = Some(args.remove(1));
        args.remove(0);
    } else if let Some(name) = args.first().and_then(|arg| arg.strip_prefix("--profile=")) {
        profile_name = Some(name.to_owned());
        args.remove(0);
    }
    if let Some(name) = &profile_name {
        profiles::validate_name(name)?;
    }
    let administrative = args.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "setup"
                | "profile"
                | "task"
                | "config"
                | "context"
                | "status"
                | "doctor"
                | "models"
                | "completions"
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
        let mut config = Config::load()?;
        let profile = profile_name.as_ref().map(|name| {
            config.profiles.get(name).cloned().with_context(|| format!("unknown profile '{name}'; create it with ai profile set {name} <provider> [model]"))
        }).transpose()?;
        if let Some(provider) = profile.as_ref().and_then(|p| p.provider.as_ref()) {
            config.default_provider = provider.clone();
        }
        let explicit_provider = args
            .first()
            .is_some_and(|arg| !arg.starts_with('-') && config.resolve(arg).is_ok());
        let (id, options) = select_request(&config, &args).unwrap_or_else(|e| e.exit());
        return request(
            &config,
            &id,
            options,
            args.is_empty(),
            profile.as_ref(),
            explicit_provider,
        );
    }
    let Some(command) = Cli::parse_from(std::iter::once("ai".to_owned()).chain(args)).command
    else {
        return Ok(());
    };
    let cwd = std::env::current_dir()?;
    let root = context::finder::find(&cwd);
    match command {
        Admin::Task { action } => match action {
            TaskAction::Set {
                name,
                instructions,
                profile,
                args,
            } => {
                tasks::set(
                    &name,
                    tasks::Task {
                        instructions,
                        profile,
                        args,
                    },
                )?;
                println!("Saved task '{name}'. Run: ai task {name}");
            }
            TaskAction::List => {
                for name in tasks::load()?.keys() {
                    println!("{name}");
                }
            }
            TaskAction::Show { name } => print!(
                "{}",
                serde_yaml_ng::to_string(tasks::load()?.get(&name).context("unknown task")?)?
            ),
            TaskAction::Remove { name } => tasks::remove(&name)?,
            TaskAction::Run(args) => {
                let task = tasks::load()?
                    .remove(&args[0])
                    .context("unknown task; use ai task list")?;
                let name = profile_name.as_ref().or(task.profile.as_ref());
                let mut config = Config::load()?;
                let profile = name
                    .map(|n| {
                        config
                            .profiles
                            .get(n)
                            .cloned()
                            .context("unknown task profile")
                    })
                    .transpose()?;
                if let Some(provider) = profile.as_ref().and_then(|p| p.provider.as_ref()) {
                    config.default_provider = provider.clone();
                }
                let mut options = Request::try_parse_from(
                    std::iter::once("ai".to_owned())
                        .chain(task.args)
                        .chain(args.into_iter().skip(1)),
                )
                .unwrap_or_else(|e| e.exit());
                options.message.insert(0, task.instructions);
                return request(
                    &config,
                    &config.default_provider,
                    options,
                    false,
                    profile.as_ref(),
                    false,
                );
            }
        },
        Admin::Setup { provider, model } => {
            if let Some(name) = profile_name {
                if provider.is_some() {
                    profiles::set(&name, provider, model, None)?;
                }
                let config = Config::load()?;
                let profile = config
                    .profiles
                    .get(&name)
                    .with_context(|| format!("unknown profile '{name}'"))?;
                println!("Profile: {name}\n{}", serde_yaml_ng::to_string(profile)?);
                return Ok(());
            }
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
        Admin::Profile { action } => match action {
            ProfileAction::Set {
                name,
                provider,
                model,
                instructions,
                instructions_file,
                install,
                bin_dir,
            } => {
                profiles::set_with_file(&name, provider, model, instructions, instructions_file)?;
                if install {
                    let path = profiles::install(&name, bin_dir.as_deref())?;
                    println!("Installed {}", path.display());
                    let parent = path
                        .parent()
                        .context("shortcut has no directory")?
                        .canonicalize()?;
                    let on_path = std::env::var_os("PATH").is_some_and(|value| {
                        std::env::split_paths(&value)
                            .any(|p| p.canonicalize().is_ok_and(|p| p == parent))
                    });
                    if !on_path {
                        eprintln!("Add {} to PATH to use this shortcut.", parent.display());
                    }
                }
                println!("Saved profile '{name}'. Run: ai --profile {name} <message>");
            }
            ProfileAction::Export { name, output } => {
                let text = profiles::export(&name, output.as_deref())?;
                if output.is_none() {
                    print!("{text}");
                }
            }
            ProfileAction::Import {
                path,
                name,
                replace,
            } => {
                profiles::import(&path, &name, replace)?;
                println!("Imported profile '{name}'. Run: ai --profile {name} <message>");
            }
            ProfileAction::List => {
                print!("{}", serde_yaml_ng::to_string(&Config::load()?.profiles)?)
            }
            ProfileAction::Remove { name } => profiles::remove(&name)?,
        },
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
            ContextAction::Explain { json, request } => {
                let config = Config::load()?;
                let profile = profile_name
                    .as_ref()
                    .map(|n| config.profiles.get(n).context("unknown profile"))
                    .transpose()?;
                let prepared = prompt::assemble(&root, &request, profile, &read_stdin()?, true)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&prepared)?);
                } else {
                    print!("{}", prepared.explain());
                }
            }
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
        Admin::Doctor {
            provider,
            json,
            timeout,
        } => {
            let report = doctor::diagnose(provider.as_deref(), profile_name.as_deref(), timeout)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.text());
            }
            if !report.healthy {
                return Err(doctor::Unhealthy.into());
            }
        }
        Admin::Status => print!(
            "{}",
            status::report(&Config::load()?, profile_name.as_deref(), &root)?
        ),
        Admin::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "ai", &mut std::io::stdout());
        }
        Admin::Provider(_) => unreachable!("handled before filesystem work"),
    }
    Ok(())
}

fn request(
    config: &Config,
    id: &str,
    options: Request,
    help_if_empty: bool,
    profile: Option<&Profile>,
    explicit_provider: bool,
) -> Result<()> {
    let mut model = config.resolve(id)?;
    if !explicit_provider && let Some(name) = profile.and_then(|p| p.model.as_ref()) {
        model.model = config.resolve_model(model.provider, name)?;
    }
    if let Some(name) = &options.model {
        model.model = config.resolve_model(model.provider, name)?;
    }
    provider::validate_args(&options.provider_arg)?;
    let cwd = std::env::current_dir()?;
    let root = context::finder::find(&cwd);
    let prepared = prompt::assemble(
        &root,
        &options,
        profile,
        &read_stdin()?,
        options.dry_run || options.explain,
    )?;
    if help_if_empty && !prepared.has_input {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    }
    anyhow::ensure!(
        prepared.has_input,
        "provide a message or pipe input: ai {id} [flags] <message>"
    );
    let request = &prepared.text;
    if options.explain {
        print!("{}", prepared.explain());
        return Ok(());
    }
    if options.dry_run {
        println!("{request}");
        return Ok(());
    }
    let binary = which::which(&model.binary)
        .with_context(|| format!("provider '{id}' binary '{}' not found", model.binary))?;
    provider::process::install_signal_handler()?;
    let limits = provider::process::Limits {
        timeout: std::time::Duration::from_secs(options.timeout),
        max_output_bytes: options.max_output_bytes,
    };
    let started = std::time::Instant::now();
    let output = provider::execute_controlled(
        &model,
        &binary,
        request,
        &cwd,
        &options.provider_arg,
        limits,
        !options.no_stream,
    )
    .with_context(|| format!("provider '{id}' failed"))?;
    if options.copy && !output.is_empty() {
        let text = String::from_utf8_lossy(&output);
        if let Err(e) = ainline::clipboard::copy(&text) {
            eprintln!("ai: warning: failed to copy output to clipboard: {e}");
        }
    }
    provider::process::write_response(output, started, limits)?;
    Ok(())
}

fn read_stdin() -> Result<String> {
    let mut text = String::new();
    if !io::stdin().is_terminal() {
        io::stdin()
            .take(storage::MAX_FILE + 1)
            .read_to_string(&mut text)?;
        anyhow::ensure!(
            text.len() as u64 <= storage::MAX_FILE,
            "stdin exceeds 1 MiB"
        );
    }
    Ok(text)
}
