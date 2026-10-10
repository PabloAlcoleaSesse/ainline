use anyhow::{Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub const MAX_FILE: u64 = 1024 * 1024;

pub fn read(path: &Path) -> Result<Option<String>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let mut value = String::new();
    file.take(MAX_FILE + 1).read_to_string(&mut value)?;
    anyhow::ensure!(
        value.len() as u64 <= MAX_FILE,
        "{} exceeds 1 MiB",
        path.display()
    );
    Ok(Some(value))
}

pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

/// OS advisory lock, automatically released on exit, including crashes.
pub fn lock(path: &Path) -> Result<File> {
    fs::create_dir_all(path.parent().context("lock has no parent")?)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
