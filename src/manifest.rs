use anyhow::{Result, bail};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub addons: HashMap<String, AddonSpec>,
}

#[derive(Deserialize)]
pub struct AddonSpec {
    pub url: String,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub subdir: Option<String>,
}

impl AddonSpec {
    /// The single ref value (tag, branch, or commit). Only meaningful after
    /// `validate` has confirmed exactly one is present and non-empty.
    pub fn rev(&self) -> &str {
        self.tag
            .as_deref()
            .or(self.branch.as_deref())
            .or(self.commit.as_deref())
            .expect("validate() guarantees exactly one ref")
    }
}

/// Validate one addon entry against the design rules. Returns the resolved
/// `subdir` so callers don't have to re-unwrap it.
pub fn validate<'a>(name: &str, spec: &'a AddonSpec) -> Result<&'a str> {
    // The name becomes a path component under addons/. Reject anything that
    // could let it escape that directory and write or delete elsewhere.
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        bail!("addon '{name}': name must not contain '/', '\\', or '..'");
    }
    if spec.url.is_empty() {
        bail!("addon '{name}': url must not be empty");
    }
    let subdir = match spec.subdir.as_deref() {
        None => bail!("addon '{name}': subdir is required"),
        Some("") => bail!("addon '{name}': subdir must not be empty"),
        Some(s) => s,
    };
    let ref_count = [&spec.tag, &spec.branch, &spec.commit]
        .iter()
        .filter(|o| o.is_some())
        .count();
    if ref_count == 0 {
        bail!("addon '{name}': one of tag, branch, or commit is required");
    }
    if ref_count > 1 {
        bail!("addon '{name}': only one of tag, branch, or commit is allowed");
    }
    if spec.rev().is_empty() {
        bail!("addon '{name}': tag/branch/commit value must not be empty");
    }
    Ok(subdir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(tag: Option<&str>, branch: Option<&str>, commit: Option<&str>) -> AddonSpec {
        AddonSpec {
            url: "https://example.com/repo".into(),
            tag: tag.map(Into::into),
            branch: branch.map(Into::into),
            commit: commit.map(Into::into),
            subdir: Some("addons/x".into()),
        }
    }

    #[test]
    fn accepts_single_ref() {
        assert_eq!(
            validate("ok", &spec(Some("v1"), None, None)).unwrap(),
            "addons/x"
        );
    }

    #[test]
    fn rejects_zero_refs() {
        assert!(validate("ok", &spec(None, None, None)).is_err());
    }

    #[test]
    fn rejects_multiple_refs() {
        assert!(validate("ok", &spec(Some("v1"), Some("main"), None)).is_err());
    }

    #[test]
    fn rejects_empty_ref_value() {
        assert!(validate("ok", &spec(Some(""), None, None)).is_err());
    }

    #[test]
    fn rejects_missing_subdir() {
        let mut s = spec(Some("v1"), None, None);
        s.subdir = None;
        assert!(validate("ok", &s).is_err());
    }

    #[test]
    fn rejects_traversal_names() {
        for name in ["..", "../evil", "a/b", "a\\b", "..\\x", ""] {
            assert!(
                validate(name, &spec(Some("v1"), None, None)).is_err(),
                "name {name:?} should be rejected"
            );
        }
    }

    #[test]
    fn rev_picks_the_present_one() {
        assert_eq!(spec(None, Some("main"), None).rev(), "main");
        assert_eq!(spec(None, None, Some("deadbeef")).rev(), "deadbeef");
    }
}
