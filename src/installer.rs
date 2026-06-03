use crate::cache::repo_cache_path;
use crate::error::GdepError;
use crate::lockfile::LockedAddon;
use git2::{Oid, Repository, TreeWalkMode, TreeWalkResult};
use std::path::{Path, PathBuf};

/// Extract an addon from the bare-clone cache into `project_dir/addons/<name>/`.
pub fn install_addon(addon: &LockedAddon, project_dir: &Path) -> Result<(), GdepError> {
    let cache_path = repo_cache_path(&addon.git)?;
    let repo = Repository::open_bare(&cache_path)?;

    let oid = Oid::from_str(&addon.commit)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    // If subdirectory is set, navigate into that subtree.
    let subtree = if let Some(subdir) = &addon.subdirectory {
        let entry = tree
            .get_path(Path::new(subdir))
            .map_err(|_| GdepError::RefNotResolved {
                addon: addon.name.clone(),
                ref_: subdir.clone(),
            })?;
        repo.find_tree(entry.id())?
    } else {
        tree
    };

    let dest = project_dir.join("addons").join(&addon.name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(GdepError::Io)?;
    }
    std::fs::create_dir_all(&dest).map_err(GdepError::Io)?;

    // Walk the subtree and write each blob to the destination.
    let mut err: Option<GdepError> = None;
    subtree.walk(TreeWalkMode::PreOrder, |root, entry| {
        if err.is_some() {
            return TreeWalkResult::Abort;
        }
        let name = match entry.name() {
            Some(n) => n,
            None => return TreeWalkResult::Skip,
        };
        let rel: PathBuf = if root.is_empty() {
            PathBuf::from(name)
        } else {
            PathBuf::from(root).join(name)
        };
        let full = dest.join(&rel);

        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                if let Err(e) = std::fs::create_dir_all(&full) {
                    err = Some(GdepError::Io(e));
                    return TreeWalkResult::Abort;
                }
            }
            Some(git2::ObjectType::Blob) => {
                let obj = match repo.find_blob(entry.id()) {
                    Ok(b) => b,
                    Err(e) => {
                        err = Some(GdepError::Git(e));
                        return TreeWalkResult::Abort;
                    }
                };
                if let Some(parent) = full.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        err = Some(GdepError::Io(e));
                        return TreeWalkResult::Abort;
                    }
                }
                if let Err(e) = std::fs::write(&full, obj.content()) {
                    err = Some(GdepError::Io(e));
                    return TreeWalkResult::Abort;
                }
            }
            _ => {}
        }
        TreeWalkResult::Ok
    })?;

    if let Some(e) = err {
        return Err(e);
    }

    Ok(())
}

/// Remove an addon's installed directory.
pub fn remove_addon(name: &str, project_dir: &Path) -> Result<(), GdepError> {
    let dest = project_dir.join("addons").join(name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(GdepError::Io)?;
    }
    Ok(())
}
