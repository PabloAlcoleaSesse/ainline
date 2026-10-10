pub mod env;
pub mod finder;
pub mod rules;
pub mod session;
use crate::storage;
use anyhow::Result;
use std::{fs, path::Path};
pub fn init(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join(".ai/rules"))?;
    let p = root.join(".ai/.gitignore");
    let _lock = storage::lock(&root.join(".ai/init.lock"))?;
    let mut content = storage::read(&p)?.unwrap_or_default();
    for rule in ["/session.json", "/*.lock", "/.tmp*", "/rules/*.local.md"] {
        if !content.lines().any(|l| l == rule) {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(rule);
            content.push('\n');
        }
    }
    storage::write(&p, content.as_bytes())
}
pub fn collect(root: &Path, git: bool) -> Result<String> {
    let mut text = env::collect(root, git);
    for (name, rule) in rules::load(root)? {
        text.push_str(&format!("\nRule ({name}):\n{rule}\n"));
    }
    for note in session::Session::load(root)?.notes {
        text.push_str(&format!("\nWorking note:\n{}\n", note.text));
    }
    Ok(text)
}
