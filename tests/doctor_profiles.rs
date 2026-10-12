use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};
fn ai(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ai"));
    cmd.current_dir(dir)
        .env("AI_CONFIG_FILE", dir.join("config.yaml"))
        .stdin(Stdio::null());
    cmd
}
#[test]
fn portable_profiles_embed_instructions_and_preserve_local_settings() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("agent.md"), "Review carefully").unwrap();
    assert!(
        ai(dir.path())
            .args([
                "profile",
                "set",
                "2",
                "cl",
                "hk4.5",
                "--instructions-file",
                "agent.md"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    let out = ai(dir.path())
        .args(["profile", "export", "2", "--output", "review.yaml"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let bundle = fs::read_to_string(dir.path().join("review.yaml")).unwrap();
    assert!(bundle.contains("claude-haiku-4-5"));
    assert!(bundle.contains("Review carefully"));
    assert!(!bundle.contains("instructions_file") && !bundle.contains("binary"));
    assert!(
        !ai(dir.path())
            .args(["profile", "export", "2", "--output", "review.yaml"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        bundle,
        fs::read_to_string(dir.path().join("review.yaml")).unwrap()
    );
    fs::remove_file(dir.path().join("agent.md")).unwrap();
    let before = ainline::config::Config::parse(
        &fs::read_to_string(dir.path().join("config.yaml")).unwrap(),
    )
    .unwrap();
    assert!(
        ai(dir.path())
            .args(["profile", "import", "review.yaml", "--name", "reviewer"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let after = ainline::config::Config::parse(
        &fs::read_to_string(dir.path().join("config.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(before.default_provider, after.default_provider);
    assert_eq!(
        serde_json::to_value(before.models).unwrap(),
        serde_json::to_value(after.models).unwrap()
    );
    let out = ai(dir.path())
        .args([
            "--profile",
            "reviewer",
            "--dry-run",
            "--no-context",
            "hello",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Review carefully"));
    assert!(
        !ai(dir.path())
            .args(["profile", "import", "review.yaml", "--name", "reviewer"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        ai(dir.path())
            .args([
                "profile",
                "import",
                "review.yaml",
                "--name",
                "reviewer",
                "--replace"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    let original = fs::read(dir.path().join("config.yaml")).unwrap();
    for invalid in [
        bundle.replace("version: 1", "version: 99"),
        format!("{bundle}\nbinary: evil\n"),
        format!("{bundle}\ninstructions_file: /tmp/private\n"),
        "x".repeat(1_048_577),
    ] {
        fs::write(dir.path().join("bad.yaml"), invalid).unwrap();
        assert!(
            !ai(dir.path())
                .args([
                    "profile",
                    "import",
                    "bad.yaml",
                    "--name",
                    "reviewer",
                    "--replace"
                ])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(original, fs::read(dir.path().join("config.yaml")).unwrap());
    }
}
#[test]
fn doctor_reports_invalid_config_and_missing_binary_as_json() {
    let dir = tempfile::tempdir().unwrap();
    for content in [
        "secret-invalid-yaml",
        "models:\n  claude:\n    provider: claude\n    model: sonnet\n    binary: /nonexistent-ai-doctor-test\n",
    ] {
        fs::write(dir.path().join("config.yaml"), content).unwrap();
        let out = ai(dir.path()).args(["doctor", "--json"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["healthy"], false);
        assert!(out.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&out.stdout).contains("secret-invalid-yaml"));
    }
}
#[cfg(unix)]
#[test]
fn doctor_bounds_probes_hides_auth_output_and_never_requests_a_model() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("provider");
    let config = format!(
        "models:\n  claude:\n    provider: claude\n    model: sonnet\n    binary: '{}'\n",
        binary.display()
    );
    fs::write(dir.path().join("config.yaml"), config).unwrap();
    for (auth, healthy) in [
        ("echo '{\"loggedIn\":true}'", true),
        ("echo '{\"loggedIn\":false}'", false),
        ("sleep 5", false),
        ("yes private", false),
    ] {
        fs::write(&binary, format!("#!/bin/sh\necho SECRET >&2\ncase \"$*\" in\n--version) echo test-1;;\n'auth status') {auth};;\n*) echo unexpected >&2; exit 99;;\nesac\n")).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let start = std::time::Instant::now();
        let out = ai(dir.path())
            .args([
                "doctor",
                "cl",
                "--json",
                "--timeout",
                if healthy { "3" } else { "1" },
            ])
            .output()
            .unwrap();
        assert!(start.elapsed().as_secs() < 4);
        assert_eq!(out.status.success(), healthy, "{:?}", out);
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["healthy"], healthy);
        assert!(out.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&out.stdout).contains("SECRET"));
    }
}
