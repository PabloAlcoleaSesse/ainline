use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn copy(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to execute pbcopy")?;

    #[cfg(target_os = "windows")]
    let mut child = Command::new("clip")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to execute clip")?;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut child = {
        if which::which("wl-copy").is_ok() {
            Command::new("wl-copy")
                .stdin(Stdio::piped())
                .spawn()
                .context("failed to execute wl-copy")?
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(Stdio::piped())
                .spawn()
                .context("failed to execute xclip")?
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    anyhow::ensure!(status.success(), "clipboard process exited with error");
    Ok(())
}
