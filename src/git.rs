use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::fs;
use std::path::Path;

// A directory is "populated" if it exists and contains at least one entry.
// Checking this instead of just exists() prevents a false "up to date" when
// the user deleted the directory contents but left the directory itself.
pub fn dir_is_populated(dir: &Path) -> bool {
    dir.read_dir().is_ok_and(|mut d| d.next().is_some())
}

// Opens an existing bare clone or creates a new one. Does NOT fetch — call
// fetch_all separately when you need the latest refs.
pub fn clone_or_open(
    url: &str,
    repo_dir: &Path,
    spinner: &ProgressBar,
) -> Result<git2::Repository> {
    if repo_dir.exists() {
        git2::Repository::open_bare(repo_dir)
            .with_context(|| format!("failed to open cache for {url}"))
    } else {
        spinner.set_message(format!("{url}: cloning"));
        git2::build::RepoBuilder::new()
            .bare(true)
            .clone(url, repo_dir)
            .with_context(|| format!("failed to clone {url}"))
    }
}

pub fn fetch_all(repo: &git2::Repository, url: &str) -> Result<()> {
    let mut remote = repo
        .find_remote("origin")
        .context("cache missing origin remote")?;
    remote
        .fetch(
            &["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"],
            None,
            None,
        )
        .with_context(|| format!("failed to fetch {url}"))?;
    Ok(())
}

pub fn resolve_ref(
    repo: &git2::Repository,
    url: &str,
    tag: Option<&str>,
    branch: Option<&str>,
    commit: Option<&str>,
) -> Result<String> {
    if let Some(t) = tag {
        let obj = repo
            .revparse_single(&format!("refs/tags/{t}"))
            .with_context(|| format!("tag '{t}' not found in {url}"))?;
        Ok(obj.peel_to_commit()?.id().to_string())
    } else if let Some(b) = branch {
        let obj = repo
            .revparse_single(&format!("refs/heads/{b}"))
            .with_context(|| format!("branch '{b}' not found in {url}"))?;
        Ok(obj.peel_to_commit()?.id().to_string())
    } else if let Some(c) = commit {
        let obj = repo
            .revparse_single(c)
            .with_context(|| format!("commit '{c}' not found in {url}"))?;
        Ok(obj.peel_to_commit()?.id().to_string())
    } else {
        unreachable!()
    }
}

pub fn copy_tree(repo: &git2::Repository, tree: &git2::Tree, dest: &Path) -> Result<()> {
    for entry in tree.iter() {
        let name = entry.name().context("entry with non-UTF-8 name")?;
        let dest_path = dest.join(name);
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                fs::create_dir_all(&dest_path)?;
                let subtree = repo.find_tree(entry.id())?;
                copy_tree(repo, &subtree, &dest_path)?;
            }
            // A symlink is stored as a blob whose content is the target path.
            // Recreate it as a symlink rather than writing the target as a
            // regular file's contents.
            Some(git2::ObjectType::Blob) if entry.filemode() == 0o120000 => {
                let blob = repo.find_blob(entry.id())?;
                #[cfg(unix)]
                {
                    use std::ffi::OsStr;
                    use std::os::unix::ffi::OsStrExt;
                    let target = OsStr::from_bytes(blob.content());
                    std::os::unix::fs::symlink(target, &dest_path).with_context(|| {
                        format!("failed to create symlink {}", dest_path.display())
                    })?;
                }
                #[cfg(not(unix))]
                {
                    eprintln!(
                        "warning: skipping symlink '{name}' — symlinks are not supported on this platform"
                    );
                }
            }
            Some(git2::ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                fs::write(&dest_path, blob.content())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if entry.filemode() & 0o111 != 0 {
                        fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))?;
                    }
                }
            }
            Some(kind) => {
                eprintln!(
                    "warning: skipping '{name}' ({kind:?}) — only regular files and directories are supported"
                );
            }
            None => {}
        }
    }
    Ok(())
}
