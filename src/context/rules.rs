use crate::storage;
use anyhow::Result;
use std::{fs, path::Path};
pub fn load(root: &Path) -> Result<Vec<(String, String)>> {
    let dir = root.join(".ai/rules");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|e| e == "md") {
            paths.push(entry.path());
        }
    }
    paths.sort();
    let mut total = 0;
    let mut rules = Vec::new();
    for p in paths {
        if let Some(text) = storage::read(&p)? {
            total += text.len();
            anyhow::ensure!(
                total as u64 <= storage::MAX_FILE,
                "rules exceed 1 MiB combined"
            );
            rules.push((p.file_name().unwrap().to_string_lossy().into_owned(), text));
        }
    }
    Ok(rules)
}
pub fn add(root: &Path, name: &str, text: &str, local: bool) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "rule name must contain only letters, digits, - or _"
    );
    anyhow::ensure!(
        !text.trim().is_empty() && text.len() as u64 <= storage::MAX_FILE,
        "invalid rule size"
    );
    super::init(root)?;
    let suffix = if local { ".local.md" } else { ".md" };
    storage::write(
        &root.join(format!(".ai/rules/{name}{suffix}")),
        text.as_bytes(),
    )
}
