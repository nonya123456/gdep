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
        git2::Oid::from_str(commit).map_err(|_| GdepError::RefNotResolved {
            addon: name.to_string(),
            ref_: commit.clone(),
        })?
    } else {
        return Err(GdepError::NoRef(name.to_string()));
    };

    Ok(oid.to_string())
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
