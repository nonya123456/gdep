use crate::error::GdepError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const MANIFEST_FILE: &str = "gdep.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddonEntry {
    pub git: String,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub subdirectory: Option<String>,
}

impl AddonEntry {
    /// Returns a human-readable description of the pinned ref.
    pub fn ref_display(&self) -> String {
        if let Some(t) = &self.tag {
            format!("tag:{t}")
        } else if let Some(b) = &self.branch {
            format!("branch:{b}")
        } else if let Some(c) = &self.commit {
            format!("commit:{}", &c[..c.len().min(8)])
        } else {
            "HEAD".to_string()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub addons: BTreeMap<String, AddonEntry>,
}

impl Manifest {
    pub fn load(dir: &Path) -> Result<Self, GdepError> {
        let path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| {
            GdepError::Manifest(format!("cannot read {}: {e}", path.display()))
        })?;
        toml::from_str(&text)
            .map_err(|e| GdepError::Manifest(format!("parse error in {}: {e}", path.display())))
    }

    pub fn save(&self, dir: &Path) -> Result<(), GdepError> {
        let path = dir.join(MANIFEST_FILE);
        let text = toml::to_string_pretty(self)
            .map_err(|e| GdepError::Manifest(format!("serialize error: {e}")))?;
        std::fs::write(&path, text).map_err(GdepError::Io)
    }

    pub fn init(dir: &Path) -> Result<(), GdepError> {
        let path = dir.join(MANIFEST_FILE);
        if path.exists() {
            return Err(GdepError::Manifest(format!(
                "{} already exists",
                path.display()
            )));
        }
        let placeholder = Manifest::default();
        let mut text = "# gdep.toml — Godot dependency manifest\n# See https://github.com/nonya123456/gdep for docs\n\n".to_string();
        text.push_str(
            &toml::to_string_pretty(&placeholder)
                .map_err(|e| GdepError::Manifest(e.to_string()))?,
        );
        std::fs::write(&path, text).map_err(GdepError::Io)
    }
}
