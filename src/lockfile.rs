use crate::error::GdepError;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const LOCK_FILE: &str = "gdep.lock";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedAddon {
    pub name: String,
    pub git: String,
    pub commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdirectory: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Lockfile {
    pub addon: Vec<LockedAddon>,
}

impl Lockfile {
    pub fn load(dir: &Path) -> Result<Self, GdepError> {
        let path = dir.join(LOCK_FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| GdepError::Lockfile(format!("cannot read {}: {e}", path.display())))?;
        toml::from_str(&text)
            .map_err(|e| GdepError::Lockfile(format!("parse error in {}: {e}", path.display())))
    }

    pub fn save(&self, dir: &Path) -> Result<(), GdepError> {
        let path = dir.join(LOCK_FILE);
        let text = toml::to_string_pretty(self)
            .map_err(|e| GdepError::Lockfile(format!("serialize error: {e}")))?;
        std::fs::write(&path, text).map_err(GdepError::Io)
    }

    pub fn find(&self, name: &str) -> Option<&LockedAddon> {
        self.addon.iter().find(|a| a.name == name)
    }

    pub fn upsert(&mut self, addon: LockedAddon) {
        if let Some(existing) = self.addon.iter_mut().find(|a| a.name == addon.name) {
            *existing = addon;
        } else {
            self.addon.push(addon);
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.addon.len();
        self.addon.retain(|a| a.name != name);
        self.addon.len() < before
    }
}
