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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_addon(name: &str) -> LockedAddon {
        LockedAddon {
            name: name.to_string(),
            git: "https://example.com/repo.git".to_string(),
            commit: "a".repeat(40),
            subdirectory: None,
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("gut"));
        lf.save(dir.path()).unwrap();

        let loaded = Lockfile::load(dir.path()).unwrap();
        assert_eq!(loaded.addon.len(), 1);
        assert_eq!(loaded.addon[0], lf.addon[0]);
    }

    #[test]
    fn load_missing_file_returns_error() {
        let dir = tempdir().unwrap();
        assert!(Lockfile::load(dir.path()).is_err());
    }

    #[test]
    fn upsert_inserts_new_entry() {
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("gut"));
        assert_eq!(lf.addon.len(), 1);
    }

    #[test]
    fn upsert_replaces_existing_entry() {
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("gut"));
        let mut updated = sample_addon("gut");
        updated.commit = "b".repeat(40);
        lf.upsert(updated);
        assert_eq!(lf.addon.len(), 1);
        assert_eq!(lf.addon[0].commit, "b".repeat(40));
    }

    #[test]
    fn remove_existing_returns_true() {
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("gut"));
        assert!(lf.remove("gut"));
        assert!(lf.addon.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut lf = Lockfile::default();
        assert!(!lf.remove("ghost"));
    }

    #[test]
    fn find_returns_correct_entry() {
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("a"));
        lf.upsert(sample_addon("b"));
        assert_eq!(lf.find("b").unwrap().name, "b");
        assert!(lf.find("c").is_none());
    }

    #[test]
    fn subdirectory_survives_roundtrip() {
        let dir = tempdir().unwrap();
        let mut lf = Lockfile::default();
        let mut addon = sample_addon("netfox");
        addon.subdirectory = Some("addons/netfox".to_string());
        lf.upsert(addon);
        lf.save(dir.path()).unwrap();

        let loaded = Lockfile::load(dir.path()).unwrap();
        assert_eq!(
            loaded.addon[0].subdirectory,
            Some("addons/netfox".to_string())
        );
    }

    #[test]
    fn none_subdirectory_omitted_from_toml() {
        let dir = tempdir().unwrap();
        let mut lf = Lockfile::default();
        lf.upsert(sample_addon("gut")); // subdirectory = None
        lf.save(dir.path()).unwrap();

        let text = std::fs::read_to_string(dir.path().join(LOCK_FILE)).unwrap();
        assert!(!text.contains("subdirectory"));
    }
}
