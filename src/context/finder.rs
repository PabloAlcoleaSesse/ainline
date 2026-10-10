use std::path::{Path, PathBuf};
/// Nearest marker wins; .git files also identify Git worktrees.
pub fn find(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|p| p.join(".git").exists() || p.join(".ai").is_dir())
        .unwrap_or(start)
        .to_path_buf()
}
