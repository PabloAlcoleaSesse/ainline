use crate::storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
pub const TTL: u64 = 24 * 60 * 60;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub text: String,
    pub created_at: u64,
}
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub notes: Vec<Note>,
}
impl Session {
    pub fn prune(&mut self, now: u64) {
        self.notes
            .retain(|n| n.created_at <= now && now - n.created_at < TTL);
    }
    pub fn load(root: &Path) -> Result<Self> {
        let p = root.join(".ai/session.json");
        if !p.exists() {
            return Ok(Self::default());
        }
        let _lock = storage::lock(&root.join(".ai/session.lock"))?;
        let mut s: Self = storage::read(&p)?
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        let old = s.notes.len();
        s.prune(storage::now());
        if s.notes.len() != old {
            storage::write(&p, &serde_json::to_vec_pretty(&s)?)?;
        }
        Ok(s)
    }
    pub fn add(root: &Path, text: String) -> Result<()> {
        super::init(root)?;
        anyhow::ensure!(!text.trim().is_empty(), "note cannot be empty");
        let _lock = storage::lock(&root.join(".ai/session.lock"))?;
        let p = root.join(".ai/session.json");
        let mut s: Self = storage::read(&p)?
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        s.prune(storage::now());
        s.notes.push(Note {
            text,
            created_at: storage::now(),
        });
        let data = serde_json::to_vec_pretty(&s)?;
        anyhow::ensure!(
            data.len() as u64 <= storage::MAX_FILE,
            "session exceeds 1 MiB"
        );
        storage::write(&p, &data)
    }
    pub fn clear(root: &Path) -> Result<()> {
        if !root.join(".ai").exists() {
            return Ok(());
        }
        let _lock = storage::lock(&root.join(".ai/session.lock"))?;
        storage::write(&root.join(".ai/session.json"), b"{\"notes\":[]}")
    }
}
