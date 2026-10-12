//! One assembly path for execution, dry-run and source inspection.
use crate::{cli::Request, context, profiles::Profile, storage};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;
#[derive(Serialize)]
pub struct Source {
    pub source: String,
    pub bytes: usize,
}
#[derive(Default, Serialize)]
pub struct Prepared {
    #[serde(skip)]
    pub text: String,
    pub sources: Vec<Source>,
    pub total_bytes: usize,
    pub has_input: bool,
}
impl Prepared {
    fn push(&mut self, source: impl Into<String>, text: &str) {
        self.sources.push(Source {
            source: source.into(),
            bytes: text.len(),
        });
        self.text.push_str(text);
        self.total_bytes += text.len();
    }
    pub fn explain(&self) -> String {
        let mut text = String::new();
        for source in &self.sources {
            text.push_str(&format!("{}\t{} bytes\n", source.source, source.bytes));
        }
        text.push_str(&format!(
            "Total: {} bytes (UTF-8, including separators; not a token estimate)\n",
            self.total_bytes
        ));
        text
    }
}
pub fn assemble(
    root: &Path,
    options: &Request,
    profile: Option<&Profile>,
    piped: &str,
    preview: bool,
) -> Result<Prepared> {
    let mut result = Prepared::default();
    if let Some(profile) = profile {
        let instructions = profile.instructions_text()?;
        if !instructions.trim().is_empty() {
            result.push(
                profile
                    .instructions_file
                    .as_ref()
                    .map(|p| format!("instructions: {}", p.display()))
                    .unwrap_or("profile instructions".into()),
                &format!("Agent instructions:\n{instructions}\n\n"),
            );
        }
    }
    if !options.no_context {
        result.push("project framing", "Project context:\n");
        result.push(
            if options.no_git {
                "environment (Git disabled)"
            } else {
                "environment and Git summary"
            },
            &context::env::collect(root, !options.no_git),
        );
        for (name, text) in context::rules::load(root)? {
            result.push(
                format!(".ai/rules/{name}"),
                &format!("\nRule ({name}):\n{text}\n"),
            );
        }
        let session = if preview {
            let mut session: context::session::Session =
                storage::read(&root.join(".ai/session.json"))?
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?
                    .unwrap_or_default();
            session.prune(storage::now());
            session
        } else {
            context::session::Session::load(root)?
        };
        for (index, note) in session.notes.iter().enumerate() {
            result.push(
                format!(".ai/session.json note {}", index + 1),
                &format!("\nWorking note:\n{}\n", note.text),
            );
        }
        result.push("request framing", "\nUser request:\n");
    }
    let mut input = Prepared::default();
    let message = options.message.join(" ");
    if !message.is_empty() {
        input.push("message / task instructions", &message);
    }
    for path in &options.file {
        anyhow::ensure!(
            path.is_file(),
            "attachment must be a regular file: {}",
            path.display()
        );
        let content = storage::read(path)?.context("attachment missing")?;
        let separator = if input.text.is_empty() { "" } else { "\n\n" };
        input.push(
            format!("file: {}", path.display()),
            &format!("{separator}File ({}):\n{content}", path.display()),
        );
    }
    anyhow::ensure!(
        !(options.no_git && (options.diff || options.staged)),
        "--no-git conflicts with --diff and --staged"
    );
    for (include, staged, label) in [
        (options.diff, false, "Git diff"),
        (options.staged, true, "Git staged diff"),
    ] {
        if include && let Some(diff) = context::env::git_diff(root, staged)? {
            let separator = if input.text.is_empty() { "" } else { "\n\n" };
            input.push(label, &format!("{separator}{label}:\n{diff}"));
        }
    }
    if !piped.is_empty() {
        let separator = if input.text.is_empty() {
            ""
        } else {
            "\n\nInput:\n"
        };
        input.push("stdin", &format!("{separator}{piped}"));
    }
    result.has_input = !input.text.trim().is_empty();
    result.text.push_str(&input.text);
    result.sources.extend(input.sources);
    result.total_bytes += input.total_bytes;
    Ok(result)
}
