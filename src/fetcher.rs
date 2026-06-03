use crate::cache::repo_cache_path;
use crate::error::GdepError;
use crate::manifest::AddonEntry;
use git2::{build::RepoBuilder, FetchOptions, RemoteCallbacks, Repository};
use indicatif::{ProgressBar, ProgressStyle};
use std::cell::Cell;

pub fn resolve_and_fetch(name: &str, entry: &AddonEntry) -> Result<String, GdepError> {
    let cache_path = repo_cache_path(&entry.git)?;

    let repo = if cache_path.exists() {
        let r = Repository::open_bare(&cache_path)?;
        fetch_all(&r, &entry.git, name)?;
        r
    } else {
        clone_bare(&entry.git, &cache_path, name)?
    };

    resolve_ref(name, &repo, entry)
}

pub fn verify_cached(name: &str, entry: &AddonEntry, commit: &str) -> Result<(), GdepError> {
    let cache_path = repo_cache_path(&entry.git)?;
    if !cache_path.exists() {
        return Err(GdepError::RefNotResolved {
            addon: name.to_string(),
            ref_: commit.to_string(),
        });
    }
    let repo = Repository::open_bare(&cache_path)?;
    repo.find_commit(git2::Oid::from_str(commit)?)?;
    Ok(())
}

fn make_fetch_options<'a>(pb: &'a ProgressBar, done: &'a Cell<bool>) -> FetchOptions<'a> {
    let mut cbs = RemoteCallbacks::new();
    cbs.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() && !done.get() {
            done.set(true);
            pb.finish_with_message("done");
        } else {
            pb.set_length(stats.total_objects() as u64);
            pb.set_position(stats.received_objects() as u64);
        }
        true
    });
    let mut opts = FetchOptions::new();
    opts.remote_callbacks(cbs);
    opts
}

fn clone_bare(url: &str, dest: &std::path::Path, name: &str) -> Result<Repository, GdepError> {
    let pb = progress_bar(&format!("Cloning {name}"));
    let done = Cell::new(false);
    let opts = make_fetch_options(&pb, &done);
    let repo = RepoBuilder::new()
        .bare(true)
        .fetch_options(opts)
        .clone(url, dest)?;
    pb.finish_with_message("done");
    Ok(repo)
}

fn fetch_all(repo: &Repository, url: &str, name: &str) -> Result<(), GdepError> {
    let pb = progress_bar(&format!("Fetching {name}"));
    let done = Cell::new(false);
    let mut opts = make_fetch_options(&pb, &done);
    let mut remote = repo.remote_anonymous(url)?;
    remote.fetch(
        &["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"],
        Some(&mut opts),
        None,
    )?;
    pb.finish_with_message("done");
    Ok(())
}

fn resolve_ref(name: &str, repo: &Repository, entry: &AddonEntry) -> Result<String, GdepError> {
    let oid = if let Some(tag) = &entry.tag {
        let refname = format!("refs/tags/{tag}");
        let r = repo
            .find_reference(&refname)
            .map_err(|_| GdepError::RefNotResolved {
                addon: name.to_string(),
                ref_: refname.clone(),
            })?;
        // peel_to_commit handles both annotated and lightweight tags
        r.peel_to_commit()
            .map_err(|_| GdepError::RefNotResolved {
                addon: name.to_string(),
                ref_: refname,
            })?
            .id()
    } else if let Some(branch) = &entry.branch {
        let refname = format!("refs/heads/{branch}");
        let r = repo
            .find_reference(&refname)
            .map_err(|_| GdepError::RefNotResolved {
                addon: name.to_string(),
                ref_: refname.clone(),
            })?;
        r.peel_to_commit()
            .map_err(|_| GdepError::RefNotResolved {
                addon: name.to_string(),
                ref_: refname,
            })?
            .id()
    } else if let Some(commit) = &entry.commit {
        repo.revparse_single(commit)
            .and_then(|obj| obj.peel_to_commit())
            .map(|c| c.id())
            .map_err(|_| GdepError::RefNotResolved {
                addon: name.to_string(),
                ref_: commit.clone(),
            })?
    } else {
        return Err(GdepError::NoRef(name.to_string()));
    };

    Ok(oid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AddonEntry;
    use crate::test_helpers;
    use tempfile::tempdir;

    fn entry_tag(url: &str, tag: &str) -> AddonEntry {
        AddonEntry { git: url.to_string(), tag: Some(tag.into()), branch: None, commit: None, subdirectory: None }
    }

    fn entry_branch(url: &str, branch: &str) -> AddonEntry {
        AddonEntry { git: url.to_string(), tag: None, branch: Some(branch.into()), commit: None, subdirectory: None }
    }

    fn entry_commit(url: &str, sha: &str) -> AddonEntry {
        AddonEntry { git: url.to_string(), tag: None, branch: None, commit: Some(sha.into()), subdirectory: None }
    }

    #[test]
    fn clone_bare_creates_bare_repo_from_local_path() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let dest_tmp = tempdir().unwrap();
        let dest = dest_tmp.path().join("cloned.git");

        let repo = clone_bare(&src.url(), &dest, "test").unwrap();

        assert!(repo.is_bare());
        assert!(dest.exists());
    }

    #[test]
    fn resolve_ref_by_tag() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let dest = tempdir().unwrap().path().join("bare.git");
        let repo = clone_bare(&src.url(), &dest, "test").unwrap();

        let sha = resolve_ref("gut", &repo, &entry_tag(&src.url(), "v1.0")).unwrap();
        assert_eq!(sha, src.commit_sha);
    }

    #[test]
    fn resolve_ref_by_branch() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let dest = tempdir().unwrap().path().join("bare.git");
        let repo = clone_bare(&src.url(), &dest, "test").unwrap();

        let sha = resolve_ref("gut", &repo, &entry_branch(&src.url(), &src.default_branch)).unwrap();
        assert_eq!(sha, src.commit_sha);
    }

    #[test]
    fn resolve_ref_by_commit() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let dest = tempdir().unwrap().path().join("bare.git");
        let repo = clone_bare(&src.url(), &dest, "test").unwrap();

        let sha = resolve_ref("gut", &repo, &entry_commit(&src.url(), &src.commit_sha)).unwrap();
        assert_eq!(sha, src.commit_sha);
    }

    #[test]
    fn fetch_all_succeeds_on_existing_bare_repo() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let dest = tempdir().unwrap().path().join("bare.git");
        let repo = clone_bare(&src.url(), &dest, "test").unwrap();

        fetch_all(&repo, &src.url(), "test").unwrap();
    }

    #[test]
    fn resolve_and_fetch_end_to_end() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let cache = tempdir().unwrap();
        test_helpers::set_test_cache(&cache);

        let sha = resolve_and_fetch("gut", &entry_tag(&src.url(), "v1.0")).unwrap();
        assert_eq!(sha, src.commit_sha);

        test_helpers::unset_test_cache();
    }

    #[test]
    fn resolve_and_fetch_second_call_uses_fetch_path() {
        let _g = test_helpers::CACHE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let src = test_helpers::make_addons_repo("gut", "v1.0");
        let cache = tempdir().unwrap();
        test_helpers::set_test_cache(&cache);

        let sha1 = resolve_and_fetch("gut", &entry_tag(&src.url(), "v1.0")).unwrap();
        let sha2 = resolve_and_fetch("gut", &entry_tag(&src.url(), "v1.0")).unwrap();
        assert_eq!(sha1, sha2);

        test_helpers::unset_test_cache();
    }
}

fn progress_bar(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template("{msg} [{bar:30.cyan/blue}] {pos}/{len} objects")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message(msg.to_string());
    pb
}
