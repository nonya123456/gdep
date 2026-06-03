use crate::cache::repo_cache_path;
use crate::error::GdepError;
use crate::lockfile::LockedAddon;
use git2::{Oid, Repository, Tree, TreeWalkMode, TreeWalkResult};
use std::path::{Path, PathBuf};

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
        write_tree(
            &repo,
            &subtree,
            &project_dir.join("addons").join(&addon.name),
        )?;
    } else if let Ok(addons_entry) = tree.get_path(Path::new("addons")) {
        let addons_tree = repo.find_tree(addons_entry.id())?;
        let base = project_dir.join("addons");
        std::fs::create_dir_all(&base).map_err(GdepError::Io)?;
        // Copy each entry individually to avoid wiping sibling addons.
        for i in 0..addons_tree.len() {
            let e = addons_tree.get(i).expect("index in bounds");
            if let (Ok(name), Some(git2::ObjectType::Tree)) = (e.name(), e.kind()) {
                write_tree(&repo, &repo.find_tree(e.id())?, &base.join(name))?;
            }
        }
    } else {
        write_tree(&repo, &tree, &project_dir.join("addons").join(&addon.name))?;
    }

    Ok(())
}

pub fn remove_addon(addon: &LockedAddon, project_dir: &Path) -> Result<(), GdepError> {
    if addon.subdirectory.is_some() {
        remove_if_exists(&project_dir.join("addons").join(&addon.name))?;
        return Ok(());
    }

    if let Some(dirs) = addon_dirs_in_cache(addon)? {
        for name in dirs {
            remove_if_exists(&project_dir.join("addons").join(name))?;
        }
        return Ok(());
    }

    remove_if_exists(&project_dir.join("addons").join(&addon.name))?;
    Ok(())
}

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
            Ok(n) => n,
            Err(_) => return TreeWalkResult::Skip,
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
        .filter_map(|e| e.name().ok().map(String::from))
        .collect();

    Ok(if dirs.is_empty() { None } else { Some(dirs) })
}

fn remove_if_exists(path: &Path) -> Result<(), GdepError> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(GdepError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::resolve_and_fetch;
    use crate::lockfile::LockedAddon;
    use crate::manifest::AddonEntry;
    use crate::test_helpers;
    use tempfile::tempdir;

    fn seed_cache(src: &test_helpers::RepoFixture, cache: &tempfile::TempDir) -> String {
        test_helpers::set_test_cache(cache);
        resolve_and_fetch(
            "test",
            &AddonEntry {
                git: src.url(),
                tag: Some("v1.0".into()),
                branch: None,
                commit: None,
                subdirectory: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn strategy_addons_dir_contents_copied() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let cache = tempdir().unwrap();
        let commit = seed_cache(&src, &cache);

        let project = tempdir().unwrap();
        install_addon(
            &LockedAddon { name: "mypkg".into(), git: src.url(), commit, subdirectory: None },
            project.path(),
        )
        .unwrap();

        assert!(project.path().join("addons/gut/plugin.gd").exists());

        test_helpers::unset_test_cache();
    }

    #[test]
    fn strategy_root_copied_to_addon_name() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_root_repo("v1.0");
        let cache = tempdir().unwrap();
        let commit = seed_cache(&src, &cache);

        let project = tempdir().unwrap();
        install_addon(
            &LockedAddon { name: "myaddon".into(), git: src.url(), commit, subdirectory: None },
            project.path(),
        )
        .unwrap();

        assert!(project.path().join("addons/myaddon/plugin.gd").exists());

        test_helpers::unset_test_cache();
    }

    #[test]
    fn strategy_subdirectory_field() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_subdir_repo("src", "v1.0");
        let cache = tempdir().unwrap();
        let commit = seed_cache(&src, &cache);

        let project = tempdir().unwrap();
        install_addon(
            &LockedAddon {
                name: "netfox".into(),
                git: src.url(),
                commit,
                subdirectory: Some("src".into()),
            },
            project.path(),
        )
        .unwrap();

        assert!(project.path().join("addons/netfox/plugin.gd").exists());

        test_helpers::unset_test_cache();
    }
}
