use std::{
    fs,
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
fn tasks_save_flags_use_profiles_and_allow_overrides() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "profile",
            "set",
            "reviewer",
            "cl",
            "--instructions",
            "Be precise",
        ],
        vec![
            "task",
            "set",
            "review",
            "--instructions",
            "Review correctness",
            "--profile",
            "reviewer",
            "--arg=--no-context",
            "--arg=--model",
            "--arg=hk4.5",
        ],
    ] {
        let out = ai(dir.path()).args(args).output().unwrap();
        assert!(out.status.success(), "{:?}", out);
    }
    let out = ai(dir.path())
        .args([
            "task",
            "review",
            "--dry-run",
            "--model",
            "sonnet",
            "this module",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Agent instructions:\nBe precise\n\nReview correctness this module\n"
    );
    assert!(
        ai(dir.path())
            .args(["task", "review", "--explain"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let before = fs::read(dir.path().join("tasks.yaml")).unwrap();
    assert!(
        !ai(dir.path())
            .args([
                "task",
                "set",
                "review",
                "--instructions",
                "bad",
                "--arg=--unknown"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(before, fs::read(dir.path().join("tasks.yaml")).unwrap());
    assert!(
        ai(dir.path())
            .args(["task", "remove", "review"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !ai(dir.path())
            .args(["task", "review"])
            .output()
            .unwrap()
            .status
            .success()
    );
}
#[test]
fn explain_matches_actual_bytes_and_does_not_prune_session_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".ai/rules")).unwrap();
    fs::write(dir.path().join(".ai/rules/style.md"), "Use café").unwrap();
    fs::write(dir.path().join("input.txt"), "hello").unwrap();
    let session = format!(
        "{{\"notes\":[{{\"text\":\"expired\",\"created_at\":0}},{{\"text\":\"current\",\"created_at\":{}}}]}}",
        ainline::storage::now()
    );
    fs::write(dir.path().join(".ai/session.json"), &session).unwrap();
    let out = ai(dir.path())
        .args([
            "context",
            "explain",
            "--json",
            "--no-git",
            "--file",
            "input.txt",
            "hi",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let dry = ai(dir.path())
        .args(["--dry-run", "--no-git", "--file", "input.txt", "hi"])
        .output()
        .unwrap();
    assert_eq!(
        report["total_bytes"].as_u64().unwrap(),
        dry.stdout.len() as u64 - 1
    );
    assert_eq!(
        report["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["bytes"].as_u64().unwrap())
            .sum::<u64>(),
        report["total_bytes"].as_u64().unwrap()
    );
    assert!(String::from_utf8_lossy(&dry.stdout).contains("current"));
    assert!(!String::from_utf8_lossy(&dry.stdout).contains("expired"));
    assert_eq!(
        fs::read_to_string(dir.path().join(".ai/session.json")).unwrap(),
        session
    );
    assert!(
        !ai(dir.path())
            .args(["context", "explain", "--no-git", "--diff"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn explicit_git_diffs_are_included_and_failures_are_visible() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(dir.path().join("example.txt"), "staged content\n").unwrap();
    assert!(
        Command::new("git")
            .args(["add", "example.txt"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(dir.path().join("example.txt"), "working content\n").unwrap();
    let out = ai(dir.path())
        .args(["context", "explain", "--json", "--diff", "--staged"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let sources = report["sources"].as_array().unwrap();
    assert!(sources.iter().any(|s| s["source"] == "Git diff"));
    assert!(sources.iter().any(|s| s["source"] == "Git staged diff"));
    let empty = tempfile::tempdir().unwrap();
    assert!(
        !ai(empty.path())
            .args(["context", "explain", "--diff"])
            .output()
            .unwrap()
            .status
            .success()
    );
}
