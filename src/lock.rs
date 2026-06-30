use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Default)]
pub struct Lockfile {
    #[serde(default)]
    pub addons: BTreeMap<String, LockedAddon>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LockedAddon {
    pub url: String,
    #[serde(default)]
    pub rev: String, // original tag/branch/commit from manifest
    pub commit: String, // resolved full SHA
    pub subdir: String,
}
