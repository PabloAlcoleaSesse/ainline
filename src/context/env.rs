use std::{path::Path, process::Command};
pub fn collect(root: &Path, git: bool) -> String {
    let stacks: Vec<_> = [
        ("go.mod", "Go"),
        ("Cargo.toml", "Rust"),
        ("package.json", "JavaScript/TypeScript"),
        ("pyproject.toml", "Python"),
    ]
    .into_iter()
    .filter_map(|(file, stack)| root.join(file).is_file().then_some(stack))
    .collect();
    let mut result = format!("Stack: {}\n", stacks.join(", "));
    if git && root.ancestors().any(|p| p.join(".git").exists()) {
        for args in [
            &["branch", "--show-current"][..],
            &["diff", "--no-ext-diff", "--stat"][..],
        ] {
            if let Ok(out) = Command::new("git")
                .arg("--no-pager")
                .args(args)
                .current_dir(root)
                .env("GIT_OPTIONAL_LOCKS", "0")
                .output()
            {
                if out.status.success() {
                    result.push_str(&String::from_utf8_lossy(&out.stdout));
                }
            }
        }
    }
    result
}

pub fn git_diff(root: &Path, staged: bool) -> anyhow::Result<Option<String>> {
    let mut command = Command::new("git");
    command.args(["--no-pager", "diff", "--no-ext-diff", "--no-textconv"]);
    if staged {
        command.arg("--staged");
    }
    command.current_dir(root).env("GIT_OPTIONAL_LOCKS", "0");
    let out = crate::provider::process::capture(
        command,
        crate::provider::process::Limits {
            timeout: std::time::Duration::from_secs(5),
            max_output_bytes: crate::storage::MAX_FILE,
        },
    )?;
    anyhow::ensure!(
        out.status.success(),
        "requested Git diff failed; run git diff locally for details"
    );
    let text = String::from_utf8(out.stdout)?.trim().to_owned();
    Ok((!text.is_empty()).then_some(text))
}
