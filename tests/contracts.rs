use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};
#[test]
fn all_native_adapters_obey_argv_stdin_output_and_exit_contracts() {
    let fixture = tempfile::tempdir().unwrap();
    let binary = fixture.path().join(if cfg!(windows) {
        "contract.exe"
    } else {
        "contract"
    });
    assert!(
        Command::new("rustc")
            .arg("--edition=2024")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/contract_provider.rs"))
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let contracts = [
        (
            "claude",
            vec![
                "--print",
                "--model",
                "test-model",
                "--output-format",
                "text",
                "--tools",
                "",
                "--no-session-persistence",
            ],
        ),
        (
            "copilot",
            vec![
                "--model",
                "test-model",
                "--silent",
                "--stream",
                "on",
                "--output-format",
                "text",
                "--no-ask-user",
                "--no-auto-update",
                "--no-color",
                "--no-custom-instructions",
                "--disable-builtin-mcps",
                "--available-tools",
            ],
        ),
        (
            "openai",
            vec![
                "--no-daemon",
                "--ask-for-approval",
                "never",
                "exec",
                "--sandbox",
                "read-only",
                "--ephemeral",
                "--skip-git-repo-check",
                "--color",
                "never",
                "--model",
                "test-model",
            ],
        ),
        ("ollama", vec!["run", "test-model"]),
        (
            "antigravity",
            vec![
                "--model",
                "test-model",
                "--mode",
                "plan",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--print-timeout",
                "5m",
            ],
        ),
    ];
    for (provider, expected) in contracts {
        let dir = tempfile::tempdir().unwrap();
        let mut config = ainline::config::Config::default();
        config.models.get_mut(provider).unwrap().binary = binary.to_string_lossy().into();
        fs::write(
            dir.path().join("config.yaml"),
            serde_yaml_ng::to_string(&config).unwrap(),
        )
        .unwrap();
        for stream in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_ai"));
            command
                .current_dir(dir.path())
                .env("AI_CONFIG_FILE", dir.path().join("config.yaml"))
                .stdin(Stdio::null())
                .args([provider, "--no-context", "--model", "test-model"]);
            if !stream {
                command.arg("--no-stream");
            }
            command.args(["literal; $(no-shell)", "café"]);
            let out = command.output().unwrap();
            assert!(out.status.success(), "{provider}: {:?}", out);
            assert_eq!(out.stdout, b"OK", "{provider}");
            let args = fs::read_to_string(dir.path().join("argv")).unwrap();
            let actual: Vec<_> = args.split('\0').collect();
            if provider == "claude" && stream {
                assert_eq!(
                    actual,
                    vec![
                        "--print",
                        "--model",
                        "test-model",
                        "--output-format",
                        "stream-json",
                        "--verbose",
                        "--include-partial-messages",
                        "--tools",
                        "",
                        "--no-session-persistence"
                    ]
                );
            } else {
                assert_eq!(actual, expected, "{provider}");
            }
            let input = fs::read_to_string(dir.path().join("stdin")).unwrap();
            if provider == "antigravity" {
                let event: serde_json::Value = serde_json::from_str(&input).unwrap();
                assert_eq!(event["message"]["content"], "literal; $(no-shell) café");
            } else {
                assert_eq!(input, "literal; $(no-shell) café");
            }
            let failed = command.env("CONTRACT_FAIL", "1").output().unwrap();
            assert_eq!(failed.status.code(), Some(42), "{provider}");
        }
    }
}
