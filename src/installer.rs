use crate::cache::repo_cache_path;
use crate::error::GdepError;
use crate::lockfile::LockedAddon;
use git2::{Oid, Repository, Tree, TreeWalkMode, TreeWalkResult};
use std::path::{Path, PathBuf};

/// Extract an addon from the bare-clone cache into the project's addons/ directory.
///
/// Install strategy (mirrors gd-plug's default behavior):
///   1. `subdirectory` set → copy that subtree to `addons/<name>/`
///   2. Repo has an `addons/` dir → copy each entry inside it directly into
///      `project/addons/` (so `repo/addons/gut/` lands at `project/addons/gut/`)
///   3. No `addons/` dir → copy repo root to `addons/<name>/`
pub fn install_addon(addon: &LockedAddon, project_dir: &Path) -> Result<(), GdepError> {
    let cache_path = repo_cache_path(&addon.git)?;
    let repo = Repository::open_bare(&cache_path)?;

    let oid = Oid::from_str(&addon.commit)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    if let Some(subdir) = &addon.subdirectory {
        let entry = tree
            .get_path(Path::new(subdir))
            .map_err(|_| GdepError::RefNotResolved {
                addon: addon.name.clone(),
                ref_: subdir.clone(),
            })?;
        let subtree = repo.find_tree(entry.id())?;
        let dest = project_dir.join("addons").join(&addon.name);
        write_tree(&repo, &subtree, &dest)?;
    } else if let Ok(addons_entry) = tree.get_path(Path::new("addons")) {
        let addons_tree = repo.find_tree(addons_entry.id())?;
        let base = project_dir.join("addons");
        std::fs::create_dir_all(&base).map_err(GdepError::Io)?;
        // Copy each top-level entry in the repo's addons/ individually so we
        // don't wipe sibling addons already installed in project/addons/.
        for i in 0..addons_tree.len() {
            let e = addons_tree.get(i).expect("index in bounds");
            if let (Some(name), Some(git2::ObjectType::Tree)) = (e.name(), e.kind()) {
                let subtree = repo.find_tree(e.id())?;
                write_tree(&repo, &subtree, &base.join(name))?;
            }
        }
    } else {
        let dest = project_dir.join("addons").join(&addon.name);
        write_tree(&repo, &tree, &dest)?;
    }

    Ok(())
}

/// Remove an addon's installed files from the project.
pub fn remove_addon(addon: &LockedAddon, project_dir: &Path) -> Result<(), GdepError> {
    if addon.subdirectory.is_some() {
        remove_if_exists(&project_dir.join("addons").join(&addon.name))?;
        return Ok(());
    }

    // Try to discover what directories were installed via auto-detection.
    if let Some(dirs) = addon_dirs_in_cache(addon)? {
        for name in dirs {
            remove_if_exists(&project_dir.join("addons").join(name))?;
        }
        return Ok(());
    }

    // Fallback for repos with no addons/ dir.
    remove_if_exists(&project_dir.join("addons").join(&addon.name))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wipe `dest` and write the full tree into it.
fn write_tree(repo: &Repository, tree: &Tree, dest: &Path) -> Result<(), GdepError> {
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(GdepError::Io)?;
    }
    std::fs::create_dir_all(dest).map_err(GdepError::Io)?;

    let mut err: Option<GdepError> = None;
    tree.walk(TreeWalkMode::PreOrder, |root, entry| {
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

/// Returns the top-level directory names found inside the repo's `addons/` tree,
/// or `None` if the cache is missing or the repo has no `addons/` directory.
fn addon_dirs_in_cache(addon: &LockedAddon) -> Result<Option<Vec<String>>, GdepError> {
    let cache_path = repo_cache_path(&addon.git)?;
    if !cache_path.exists() {
        return Ok(None);
    }
    let repo = Repository::open_bare(&cache_path)?;
    let oid = Oid::from_str(&addon.commit)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let addons_entry = match tree.get_path(Path::new("addons")) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    let addons_tree = repo.find_tree(addons_entry.id())?;

    let dirs: Vec<String> = (0..addons_tree.len())
        .filter_map(|i| addons_tree.get(i))
        .filter(|e| matches!(e.kind(), Some(git2::ObjectType::Tree)))
        .filter_map(|e| e.name().map(String::from))
        .collect();

    Ok(if dirs.is_empty() { None } else { Some(dirs) })
}

fn remove_if_exists(path: &Path) -> Result<(), GdepError> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(GdepError::Io)?;
    }
    Ok(())
}
