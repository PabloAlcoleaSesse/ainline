use ainline::{
    cli::{Admin, Cli, Request},
    config::Config,
    context::{
        self,
        session::{Note, Session, TTL},
    },
    provider::{self, Provider},
    storage,
};
use clap::Parser;
use std::{fs, path::Path};

#[test]
fn provider_selection_and_request_flags() {
    assert!(matches!(
        Cli::try_parse_from(["ai", "status"]).unwrap().command,
        Some(Admin::Status)
    ));
    assert!(Cli::try_parse_from(["ai", "config", "typo"]).is_err());
    let cli =
        Cli::try_parse_from(["ai", "claude", "--model", "sonnet", "review", "--help"]).unwrap();
    let Some(Admin::Provider(args)) = cli.command else {
        panic!("expected provider");
    };
    assert_eq!(args[0], "claude");
    let request = Request::try_parse_from(&args).unwrap();
    assert_eq!(request.model.as_deref(), Some("sonnet"));
    assert_eq!(request.message, ["review", "--help"]);
    let request = Request::try_parse_from(["copilot", "--dry-run", "--", "--help"]).unwrap();
    assert!(request.dry_run);
    assert_eq!(request.message, ["--help"]);
    assert!(Request::try_parse_from(["claude", "--typo", "message"]).is_err());
    assert!(Request::try_parse_from(["claude", "--task", "general", "message"]).is_err());
    let request = Request::try_parse_from([
        "claude",
        "--provider-arg=--effort",
        "--provider-arg=high",
        "hello",
    ])
    .unwrap();
    assert_eq!(request.provider_arg, ["--effort", "high"]);
}

#[test]
fn nearest_root_supports_git_worktree_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".git"), "gitdir: somewhere").unwrap();
    let nested = dir.path().join("a/b");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(context::finder::find(&nested), dir.path());
    fs::create_dir_all(dir.path().join("a/.ai")).unwrap();
    assert_eq!(context::finder::find(&nested), dir.path().join("a"));
}

#[test]
fn ttl_expires_at_boundary_and_rejects_future_notes() {
    let now = TTL * 2;
    let mut s = Session {
        notes: vec![
            Note {
                text: "expired".into(),
                created_at: now - TTL,
            },
            Note {
                text: "live".into(),
                created_at: now - TTL + 1,
            },
            Note {
                text: "future".into(),
                created_at: now + 1,
            },
        ],
    };
    s.prune(now);
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.notes[0].text, "live");
}

#[test]
fn context_persistence_and_safe_rule_names() {
    let dir = tempfile::tempdir().unwrap();
    Session::add(dir.path(), "working".into()).unwrap();
    assert_eq!(Session::load(dir.path()).unwrap().notes[0].text, "working");
    context::rules::add(dir.path(), "style", "Use Rust", false).unwrap();
    context::rules::add(dir.path(), "private", "Personal", true).unwrap();
    assert!(context::rules::add(dir.path(), "../outside", "no", false).is_err());
    assert_eq!(context::rules::load(dir.path()).unwrap().len(), 2);
    let ignore = fs::read_to_string(dir.path().join(".ai/.gitignore")).unwrap();
    assert!(ignore.contains("/session.json"));
    assert!(ignore.contains("/rules/*.local.md"));
    assert!(!ignore.lines().any(|line| line == "rules/"));
    Session::clear(dir.path()).unwrap();
    assert!(Session::load(dir.path()).unwrap().notes.is_empty());
}

#[test]
fn expired_notes_are_removed_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    context::init(dir.path()).unwrap();
    let path = dir.path().join(".ai/session.json");
    storage::write(&path, br#"{"notes":[{"text":"stale","created_at":0}]}"#).unwrap();
    assert!(Session::load(dir.path()).unwrap().notes.is_empty());
    assert!(!fs::read_to_string(path).unwrap().contains("stale"));
}

#[test]
fn corrupt_state_is_not_silently_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    context::init(dir.path()).unwrap();
    let path = dir.path().join(".ai/session.json");
    fs::write(&path, "bad json").unwrap();
    assert!(Session::add(dir.path(), "note".into()).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "bad json");
}

#[test]
fn config_validates_aliases_and_preserves_legacy_models() {
    let mut config = Config::default();
    config.validate().unwrap();
    assert_eq!(config.resolve("local").unwrap().binary, "ollama");
    assert!(config.resolve("unknown").is_err());
    config.models.get_mut("ollama").unwrap().model.clear();
    assert!(config.validate().is_err());
    let old = "models:\n  local:\n    provider: ollama\n    model: custom-model\n    binary: custom-bin\n    daily_requests: 0\n    daily_budget_usd: 0\n    estimated_cost_usd: 0.1\npriorities:\n  general: [local]\n";
    let config = Config::parse(old).unwrap();
    assert_eq!(config.resolve("local").unwrap().model, "custom-model");
    assert_eq!(config.resolve("copilot").unwrap().binary, "copilot");
    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(!yaml.contains("priorities"));
    assert!(!yaml.contains("daily_requests"));
    assert!(Config::parse(&old.replace("binary:", "binnary:")).is_err());
    assert!(Config::parse(&old.replace("  local:", "  status:")).is_err());
}

#[test]
fn openai_body_and_response_handle_special_text_and_reasoning() {
    let p = provider::openai::OpenAi;
    let body: serde_json::Value =
        serde_json::from_str(&p.input("model", "colon: {foo}\n$(touch /tmp/no)")).unwrap();
    assert_eq!(body["input"], "colon: {foo}\n$(touch /tmp/no)");
    let text = p.output(br#"{"status":"completed","output":[{"type":"reasoning"},{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}"#.to_vec()).unwrap();
    assert_eq!(text, b"ok\n");
    assert!(p.output(br#"{"status":"failed"}"#.to_vec()).is_err());
    let args: Vec<_> = p
        .command(Path::new("openai"), "model")
        .get_args()
        .map(|s| s.to_owned())
        .collect();
    assert_eq!(args, ["responses", "create"]);
}

#[test]
fn environment_detects_multiple_stacks() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["go.mod", "Cargo.toml", "package.json", "pyproject.toml"] {
        fs::write(dir.path().join(f), "").unwrap();
    }
    let env = context::env::collect(dir.path(), false);
    for stack in ["Go", "Rust", "JavaScript/TypeScript", "Python"] {
        assert!(env.contains(stack));
    }
}

#[test]
fn provider_and_model_shorthands_are_explicit_and_provider_specific() {
    use ainline::{aliases, config::Kind};
    let config = Config::default();
    for (short, full) in [
        ("cl", "claude"),
        ("cp", "copilot"),
        ("ag", "antigravity"),
        ("oa", "openai"),
        ("ol", "ollama"),
    ] {
        assert_eq!(
            config.resolve(short).unwrap().binary,
            config.resolve(full).unwrap().binary
        );
    }
    assert_eq!(
        config.resolve_model(Kind::Claude, "hk4.5").unwrap(),
        "claude-haiku-4-5-20251001"
    );
    assert_eq!(
        config.resolve_model(Kind::Copilot, "hk4.5").unwrap(),
        "claude-haiku-4.5"
    );
    assert!(config.resolve_model(Kind::Ollama, "hk4.5").is_err());
    assert!(config.resolve_model(Kind::Claude, "hk99.9").is_err());
    assert_eq!(
        config
            .resolve_model(Kind::Claude, "full-custom-model-id")
            .unwrap(),
        "full-custom-model-id"
    );
    for value in ["hk4.5", "qw2", "gp3.1"] {
        assert!(aliases::is_model_code(value));
    }
    for value in [
        "",
        "h",
        "hk",
        "hk4.",
        "hk.4",
        "hk4..5",
        "HK4.5",
        "hk4.5 explain",
        "你好",
        "é4.5",
    ] {
        assert!(!aliases::is_model_code(value), "{value}");
    }
}

#[test]
fn positional_model_preserves_message_flags_and_literal_escape() {
    let parse = |args: &[&str]| {
        Request::try_from_provider_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    };
    let request = parse(&["cl", "hk4.5", "--no-git", "what", "is", "this", "--help"]).unwrap();
    assert_eq!(request.model.as_deref(), Some("hk4.5"));
    assert!(request.no_git);
    assert_eq!(request.message, ["what", "is", "this", "--help"]);
    let request = parse(&["claude", "--", "hk4.5", "explain"]).unwrap();
    assert!(request.model.is_none());
    assert_eq!(request.message, ["hk4.5", "explain"]);
    let request = parse(&["cl", "hk4.5 explain"]).unwrap();
    assert!(request.model.is_none());
    assert_eq!(request.message, ["hk4.5 explain"]);
    assert!(parse(&["cl", "hk4.5", "--model", "sonnet", "explain"]).is_err());
    assert!(parse(&["cl", "hk4.5", "--typo", "explain"]).is_err());
    assert!(!parse(&["cl", "hk4.5", "--help"]).unwrap_err().use_stderr());
}

#[test]
fn configured_model_shorthands_override_defaults_and_exact_provider_aliases_win() {
    use ainline::config::Kind;
    let config = Config::parse("models:\n  cl:\n    provider: ollama\n    model: private\n    binary: private-ollama\nmodel_aliases:\n  claude:\n    hk4.5: private-haiku-deployment\n    sn9: future-private-model\n").unwrap();
    assert_eq!(config.resolve("cl").unwrap().binary, "private-ollama");
    assert_eq!(
        config.resolve_model(Kind::Claude, "hk4.5").unwrap(),
        "private-haiku-deployment"
    );
    assert_eq!(
        config.resolve_model(Kind::Claude, "sn9").unwrap(),
        "future-private-model"
    );
    assert!(config.resolve_model(Kind::Claude, "op4.5").is_ok());
    assert!(Config::parse("model_aliases:\n  claude:\n    hk4.: invalid\n").is_err());
    assert!(Config::parse("model_aliases:\n  typo:\n    hk4.5: invalid\n").is_err());
    assert!(Config::parse("model_aliases:\n  claude:\n    hk4.5: ''\n").is_err());
}

#[test]
fn default_provider_selection_preserves_plain_messages_and_explicit_overrides() {
    use ainline::cli::select_request;
    let config = Config {
        default_provider: "copilot".into(),
        ..Config::default()
    };
    let select = |args: &[&str]| {
        select_request(
            &config,
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
        .unwrap()
    };
    let (id, request) = select(&["explain", "this", "code"]);
    assert_eq!(id, "copilot");
    assert_eq!(request.message, ["explain", "this", "code"]);
    let (id, request) = select(&["oa", "explain", "this"]);
    assert_eq!(id, "oa");
    assert!(request.model.is_none());
    let (id, request) = select(&["cl", "hk4.5", "explain"]);
    assert_eq!(id, "cl");
    assert_eq!(request.model.as_deref(), Some("hk4.5"));
    let (id, request) = select(&["--dry-run", "--", "oa", "is", "text"]);
    assert_eq!(id, "copilot");
    assert_eq!(request.message, ["oa", "is", "text"]);
    let (id, request) = select(&["--model", "hk4.5", "hello"]);
    assert_eq!(id, "copilot");
    assert_eq!(request.model.as_deref(), Some("hk4.5"));
    let (_, request) = select(&["hk4.5", "is", "text"]);
    assert!(request.model.is_none());
    assert_eq!(request.message[0], "hk4.5");
    assert!(Config::parse("default_provider: misspelled\n").is_err());
    assert_eq!(
        Config::parse("models: {}\n").unwrap().default_provider,
        "claude"
    );
}
