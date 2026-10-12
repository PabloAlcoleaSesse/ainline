use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
fn ai(dir: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ai"));
    c.current_dir(dir)
        .env("AI_CONFIG_FILE", dir.join("config.yaml"))
        .stdin(Stdio::null());
    c
}

#[test]
fn config_init_never_overwrites_and_stdin_is_context() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        ai(dir.path())
            .args(["config", "init"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let original = fs::read(dir.path().join("config.yaml")).unwrap();
    assert!(
        !ai(dir.path())
            .args(["config", "init"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(original, fs::read(dir.path().join("config.yaml")).unwrap());
    let mut child = ai(dir.path())
        .args(["copilot", "--dry-run", "--no-context", "explain"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"hello from pipe")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "explain\n\nInput:\nhello from pipe\n"
    );
    assert!(!dir.path().join("usage.json").exists());
}

#[test]
fn dry_run_keeps_context_and_no_context_excludes_it() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        ai(dir.path())
            .args(["context", "rule", "style", "Use typed errors"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        ai(dir.path())
            .args(["context", "note", "Migrating login"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let out = ai(dir.path())
        .current_dir(&nested)
        .args(["claude", "--dry-run", "--no-git", "review"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let prompt = String::from_utf8_lossy(&out.stdout);
    assert!(prompt.contains("Use typed errors"));
    assert!(prompt.contains("Migrating login"));
    assert!(prompt.contains("User request:\nreview"));
    let out = ai(dir.path())
        .args(["claude", "--dry-run", "--no-context", "review"])
        .output()
        .unwrap();
    assert_eq!(out.stdout, b"review\n");
    assert!(!dir.path().join("usage.json").exists());
}

#[test]
fn missing_message_invalid_flags_and_help_are_unambiguous() {
    let dir = tempfile::tempdir().unwrap();
    for args in [vec!["claude"], vec!["claude", "--typo", "hello"]] {
        assert!(!ai(dir.path()).args(args).output().unwrap().status.success());
    }
    for args in [vec![], vec!["--help"], vec!["claude", "--help"]] {
        assert!(ai(dir.path()).args(args).output().unwrap().status.success());
    }
    let help = ai(dir.path()).args(["claude", "--help"]).output().unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage: ai claude"));
    let mut child = ai(dir.path())
        .args(["ollama", "--dry-run", "--no-context"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin-only request")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"stdin-only request\n");
}

#[cfg(unix)]
fn script(dir: &std::path::Path, name: &str, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
#[test]
fn direct_selection_forwards_arguments_model_stdin_and_exit_codes_without_fallback() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.models.get_mut("ollama").unwrap().binary =
        script(dir.path(), "selected", "printf '%s\\n' \"$@\"; cat");
    config.models.get_mut("claude").unwrap().binary =
        script(dir.path(), "unused", "touch unexpected; exit 99");
    config.models.get_mut("copilot").unwrap().binary = script(
        dir.path(),
        "fail",
        "cat >/dev/null; echo provider-failed >&2; exit 42",
    );
    config.models.get_mut("antigravity").unwrap().binary =
        script(dir.path(), "stop", "cat >/dev/null; exit 130");
    config.models.get_mut("openai").unwrap().binary = dir
        .path()
        .join("not-installed")
        .to_string_lossy()
        .into_owned();
    fs::write(
        dir.path().join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    // Obsolete (even corrupt) ledgers must not affect the direct invocation path.
    fs::write(dir.path().join("usage.json"), "not json").unwrap();
    let out = ai(dir.path())
        .args([
            "ollama",
            "--no-context",
            "--model",
            "custom",
            "--provider-arg=--verbose",
            "hello ; $(echo literal)",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "run\ncustom\n--verbose\nhello ; $(echo literal)"
    );
    assert!(out.stderr.is_empty());
    let out = ai(dir.path())
        .args(["copilot", "--no-context", "hello"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(42));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("provider-failed"));
    let out = ai(dir.path())
        .args(["antigravity", "--no-context", "hello"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(130));
    let out = ai(dir.path())
        .args(["openai", "--no-context", "hello"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!dir.path().join("unexpected").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("usage.json")).unwrap(),
        "not json"
    );
    assert!(!dir.path().join("usage.lock").exists());
}

#[cfg(unix)]
#[test]
fn short_and_long_forms_invoke_the_same_model_with_identical_context() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.models.get_mut("claude").unwrap().binary =
        script(dir.path(), "claude-mock", "printf '%s\\n' \"$@\"; cat");
    config.models.get_mut("copilot").unwrap().binary =
        script(dir.path(), "copilot-mock", "printf '%s\\n' \"$@\"; cat");
    fs::write(
        dir.path().join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    assert!(
        ai(dir.path())
            .args(["context", "note", "working note"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let short = ai(dir.path())
        .args([
            "cl",
            "hk4.5",
            "--no-stream",
            "--no-git",
            "what",
            "is",
            "this",
        ])
        .output()
        .unwrap();
    let long = ai(dir.path())
        .args([
            "claude",
            "--no-stream",
            "--no-git",
            "--model",
            "claude-haiku-4-5-20251001",
            "what is this",
        ])
        .output()
        .unwrap();
    assert!(
        short.status.success(),
        "{}",
        String::from_utf8_lossy(&short.stderr)
    );
    assert!(long.status.success());
    assert_eq!(short.stdout, long.stdout);
    assert!(String::from_utf8_lossy(&short.stdout).contains("working note"));
    let other = ai(dir.path())
        .args(["cp", "hk4.5", "--no-context", "hello"])
        .output()
        .unwrap();
    assert!(other.status.success());
    assert!(String::from_utf8_lossy(&other.stdout).contains("claude-haiku-4.5\n"));
    let mut child = ai(dir.path())
        .args(["cl", "hk4.5", "--dry-run", "--no-context"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"message via stdin")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"message via stdin\n");
}

#[test]
fn model_listing_unknown_codes_and_escaped_text() {
    let dir = tempfile::tempdir().unwrap();
    let out = ai(dir.path()).args(["models", "cl"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("hk4.5\tclaude-haiku-4-5-20251001"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("Copilot"));
    for args in [
        vec!["cl", "hk99.9", "--dry-run", "hello"],
        vec!["ol", "hk4.5", "--dry-run", "hello"],
        vec!["cl", "hk4.5", "--model", "sonnet", "hello"],
    ] {
        assert!(!ai(dir.path()).args(args).output().unwrap().status.success());
    }
    let out = ai(dir.path())
        .args(["cl", "--dry-run", "--no-context", "--", "hk4.5", "is text"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"hk4.5 is text\n");
}

#[test]
fn setup_saves_defaults_and_model_updates_preserve_other_settings() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    assert!(
        ai(dir.path())
            .args(["setup", "cl", "hk4.5"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let path = dir.path().join("config.yaml");
    let config = Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(config.default_provider, "claude");
    assert_eq!(
        config.resolve("cl").unwrap().model,
        "claude-haiku-4-5-20251001"
    );
    assert!(
        ai(dir.path())
            .args(["config", "model", "oa", "custom-codex-model"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let config = Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(config.default_provider, "claude");
    assert_eq!(config.resolve("oa").unwrap().model, "custom-codex-model");
    assert_eq!(
        config.resolve("cl").unwrap().model,
        "claude-haiku-4-5-20251001"
    );
    let saved = fs::read(&path).unwrap();
    for args in [
        vec!["setup", "typo"],
        vec!["config", "model", "oa", "hk4.5"],
    ] {
        assert!(!ai(dir.path()).args(args).output().unwrap().status.success());
        assert_eq!(fs::read(&path).unwrap(), saved);
    }
    let shown = ai(dir.path()).args(["setup"]).output().unwrap();
    assert!(shown.status.success());
    assert!(String::from_utf8_lossy(&shown.stdout).contains("Default provider: claude"));
    assert_eq!(fs::read(&path).unwrap(), saved);
    let out = ai(dir.path())
        .args(["--dry-run", "--no-context", "--", "config", "is", "text"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"config is text\n");
}

#[cfg(unix)]
#[test]
fn plain_messages_and_provider_only_requests_use_saved_defaults() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.models.get_mut("claude").unwrap().binary =
        script(dir.path(), "claude-default", "printf '%s\\n' \"$@\"; cat");
    config.models.get_mut("openai").unwrap().binary = script(
        dir.path(),
        "openai-default",
        "printf '%s\\n' \"$@\" > received-args.txt; cat > received.txt; printf 'ok\\n'",
    );
    fs::write(
        dir.path().join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    assert!(
        ai(dir.path())
            .args(["setup", "cl", "hk4.5"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let short = ai(dir.path())
        .args(["--no-stream", "explain", "this"])
        .output()
        .unwrap();
    let explicit = ai(dir.path())
        .args(["cl", "hk4.5", "--no-stream", "explain", "this"])
        .output()
        .unwrap();
    assert!(short.status.success());
    assert_eq!(short.stdout, explicit.stdout);
    assert!(String::from_utf8_lossy(&short.stdout).contains("claude-haiku-4-5-20251001"));
    let out = ai(dir.path())
        .args(["oa", "explain", "this"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let body = fs::read_to_string(dir.path().join("received.txt")).unwrap();
    assert!(body.contains("User request:\nexplain this"));
    let args = fs::read_to_string(dir.path().join("received-args.txt")).unwrap();
    assert!(args.contains("exec\n"));
    assert!(!args.contains("--model"));
    let out = ai(dir.path())
        .args(["codex", "--model", "chosen-model", "--no-context", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"ok\n");
    let args = fs::read_to_string(dir.path().join("received-args.txt")).unwrap();
    assert!(args.contains("--model\nchosen-model\n"));
    let mut child = ai(dir.path())
        .arg("--no-stream")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin-only defaults")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("stdin-only defaults"));
}

#[cfg(unix)]
#[test]
fn successful_provider_warnings_reach_stderr() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.models.get_mut("ollama").unwrap().binary = script(
        dir.path(),
        "warning",
        "cat >/dev/null; echo notice >&2; echo answer",
    );
    fs::write(
        dir.path().join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    let out = ai(dir.path())
        .args(["ol", "--no-context", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"answer\n");
    assert_eq!(out.stderr, b"notice\n");
}

#[test]
fn profile_updates_are_isolated_validated_and_preserve_instructions() {
    use ainline::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let out = ai(dir.path()).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&[
        "profile",
        "set",
        "2",
        "cl",
        "hk4.5",
        "--instructions",
        "Review carefully",
    ]);
    run(&["profile", "set", "reviewer", "ol", "qw2.5"]);
    run(&["--profile", "2", "setup", "cl", "sn4.5"]);
    let path = dir.path().join("config.yaml");
    let config = Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(config.default_provider, "claude");
    assert_eq!(config.models["claude"].model, "sonnet");
    assert_eq!(config.profiles["2"].model.as_deref(), Some("sn4.5"));
    assert_eq!(config.profiles["2"].instructions, "Review carefully");
    assert_eq!(config.profiles["reviewer"].model.as_deref(), Some("qw2.5"));
    let before = fs::read(&path).unwrap();
    assert!(
        !ai(dir.path())
            .args(["profile", "set", "2", "missing"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(before, fs::read(&path).unwrap());
    run(&["profile", "set", "2", "ol"]);
    let config = Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(config.profiles["2"].model.is_none());
    assert_eq!(config.profiles["2"].instructions, "Review carefully");
    run(&["profile", "set", "2", "--instructions", ""]);
    let config = Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(config.profiles["2"].instructions.is_empty());
    assert!(String::from_utf8_lossy(&run(&["profile", "list"]).stdout).contains("reviewer"));
    run(&["profile", "remove", "2"]);
    assert!(
        !ai(dir.path())
            .args(["--profile", "2", "--dry-run", "hello"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn profile_instructions_compose_with_project_context_and_literal_messages() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "profile",
            "set",
            "reviewer",
            "--instructions",
            "Check edge cases",
        ],
        vec!["context", "rule", "style", "Prefer typed errors"],
    ] {
        assert!(ai(dir.path()).args(args).output().unwrap().status.success());
    }
    let out = ai(dir.path())
        .args(["--profile=reviewer", "--dry-run", "--no-git", "review"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let prompt = String::from_utf8_lossy(&out.stdout);
    assert!(prompt.starts_with("Agent instructions:\nCheck edge cases\n\nProject context:"));
    assert!(prompt.contains("Prefer typed errors"));
    let out = ai(dir.path())
        .args([
            "--profile",
            "reviewer",
            "--dry-run",
            "--no-context",
            "--",
            "oa",
            "--profile",
            "literal",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.stdout,
        b"Agent instructions:\nCheck edge cases\n\noa --profile literal\n"
    );
    let out = ai(dir.path())
        .args(["--dry-run", "--no-context", "--", "--profile", "reviewer"])
        .output()
        .unwrap();
    assert_eq!(out.stdout, b"--profile reviewer\n");
    let mut child = ai(dir.path())
        .args(["--profile", "reviewer", "--dry-run", "--no-context"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"piped request")
        .unwrap();
    assert_eq!(
        child.wait_with_output().unwrap().stdout,
        b"Agent instructions:\nCheck edge cases\n\npiped request\n"
    );
    let out = ai(dir.path()).args(["--profile"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn numbered_invocation_and_explicit_overrides_select_expected_models() {
    use ainline::{config::Config, profiles::Profile};
    use std::os::unix::process::CommandExt;
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.models.get_mut("ollama").unwrap().binary =
        script(dir.path(), "local", "printf '%s\\n' \"$@\"; cat");
    config.models.get_mut("claude").unwrap().binary =
        script(dir.path(), "remote", "printf '%s\\n' \"$@\"; cat");
    config.profiles.insert(
        "27".into(),
        Profile {
            provider: Some("ol".into()),
            model: Some("custom-profile-model".into()),
            instructions: "Review carefully".into(),
            ..Profile::default()
        },
    );
    config.profiles.insert(
        "2".into(),
        Profile {
            provider: Some("ol".into()),
            model: Some("other-profile-model".into()),
            instructions: String::new(),
            ..Profile::default()
        },
    );
    fs::write(
        dir.path().join("config.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    for (args, expected) in [
        (
            vec!["--no-context", "hello"],
            "run\ncustom-profile-model\nAgent instructions:\nReview carefully\n\nhello",
        ),
        (
            vec!["--no-context", "--model", "explicit", "hello"],
            "run\nexplicit\nAgent instructions:\nReview carefully\n\nhello",
        ),
        (
            vec!["ol", "--no-context", "hello"],
            "run\nqwen2.5-coder:7b\nAgent instructions:\nReview carefully\n\nhello",
        ),
        (
            vec!["ol", "qw2.5", "--no-context", "hello"],
            "run\nqwen2.5-coder:7b\nAgent instructions:\nReview carefully\n\nhello",
        ),
        (
            vec!["--profile", "2", "--no-context", "hello"],
            "run\nother-profile-model\nhello",
        ),
    ] {
        let out = ai(dir.path()).arg0("ai27").args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    }
    let out = ai(dir.path())
        .arg0("ai27")
        .args(["cl", "hk4.5", "--no-stream", "--no-context", "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("claude-haiku-4-5-20251001"));
    assert!(text.contains("Agent instructions:\nReview carefully"));
    let out = ai(dir.path())
        .arg0("ai27")
        .args(["setup", "ol", "changed-model"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let saved =
        Config::parse(&fs::read_to_string(dir.path().join("config.yaml")).unwrap()).unwrap();
    assert_eq!(saved.profiles["27"].model.as_deref(), Some("changed-model"));
    assert_eq!(saved.models["ollama"].model, "qwen2.5-coder:7b");
}

#[test]
fn instruction_files_reload_status_is_profile_aware_and_inline_replaces_file() {
    let dir = tempfile::tempdir().unwrap();
    let instructions = dir.path().join("reviewer.md");
    fs::write(&instructions, "First role").unwrap();
    let out = ai(dir.path())
        .args([
            "profile",
            "create",
            "2",
            "cl",
            "hk4.5",
            "--instructions-file",
            "reviewer.md",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::write(&instructions, "Updated role").unwrap();
    assert!(
        ai(dir.path())
            .args(["context", "rule", "team", "Team rules"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let out = ai(dir.path())
        .args(["--profile", "2", "--dry-run", "--no-git", "review"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Updated role")
            && text.contains("Team rules")
            && !text.contains("First role")
    );
    let out = ai(dir.path())
        .args(["--profile", "2", "status"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for expected in [
        "Profile: 2",
        "Effective provider: cl",
        "claude-haiku-4-5-20251001",
        "reviewer.md",
        "Updated role",
        "team.md",
        "session.json",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    let global = ai(dir.path()).arg("status").output().unwrap();
    assert!(!String::from_utf8_lossy(&global.stdout).contains("Updated role"));
    fs::remove_file(&instructions).unwrap();
    let out = ai(dir.path())
        .args(["--profile", "2", "--dry-run", "hello"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("instructions file missing"));
    assert!(
        ai(dir.path())
            .args([
                "profile",
                "set",
                "2",
                "--instructions",
                "Inline replacement"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    let out = ai(dir.path())
        .args(["--profile", "2", "--dry-run", "--no-context", "hello"])
        .output()
        .unwrap();
    assert_eq!(
        out.stdout,
        b"Agent instructions:\nInline replacement\n\nhello\n"
    );
    fs::write(&instructions, vec![b'x'; 1_048_577]).unwrap();
    let config_before = fs::read(dir.path().join("config.yaml")).unwrap();
    assert!(
        !ai(dir.path())
            .args(["profile", "set", "2", "--instructions-file", "reviewer.md"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        config_before,
        fs::read(dir.path().join("config.yaml")).unwrap()
    );
    assert!(
        !ai(dir.path())
            .args([
                "profile",
                "set",
                "2",
                "--instructions",
                "inline",
                "--instructions-file",
                "reviewer.md"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn profile_installer_creates_working_shortcuts_without_overwriting_commands() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin with spaces");
    for name in ["2", "reviewer"] {
        let out = ai(dir.path())
            .args([
                "profile",
                "create",
                name,
                "ol",
                "qw2.5",
                "--instructions",
                "Review carefully",
                "--install",
                "--bin-dir",
            ])
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let command = if name == "2" { "ai2" } else { "ai-reviewer" };
        let shortcut = bin.join(if cfg!(windows) {
            format!("{command}.cmd")
        } else {
            command.into()
        });
        let out = Command::new(shortcut)
            .current_dir(dir.path())
            .env("AI_CONFIG_FILE", dir.path().join("config.yaml"))
            .args(["--dry-run", "--no-context", "hello"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            out.stdout,
            b"Agent instructions:\nReview carefully\n\nhello\n"
        );
        // Reinstalling an unchanged shortcut is idempotent.
        assert!(
            ai(dir.path())
                .args(["profile", "set", name, "--install", "--bin-dir"])
                .arg(&bin)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let occupied = bin.join(if cfg!(windows) { "ai3.cmd" } else { "ai3" });
    fs::write(&occupied, "existing command").unwrap();
    let out = ai(dir.path())
        .args(["profile", "create", "3", "--install", "--bin-dir"])
        .arg(&bin)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(fs::read_to_string(occupied).unwrap(), "existing command");
}

#[test]
fn file_attachments_and_completions_work() {
    let dir = tempfile::tempdir().unwrap();
    let sample_file = dir.path().join("sample.txt");
    fs::write(&sample_file, "content of sample file").unwrap();

    let out = ai(dir.path())
        .args([
            "copilot",
            "--dry-run",
            "--no-context",
            "-f",
            sample_file.to_str().unwrap(),
            "explain this file",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let prompt = String::from_utf8_lossy(&out.stdout);
    assert!(prompt.contains("File ("));
    assert!(prompt.contains("content of sample file"));
    assert!(prompt.contains("explain this file"));

    let comp = ai(dir.path())
        .args(["completions", "zsh"])
        .output()
        .unwrap();
    assert!(comp.status.success());
    let script = String::from_utf8_lossy(&comp.stdout);
    assert!(script.contains("#compdef ai"));
}
