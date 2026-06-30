use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cache::{cache_dir, url_to_key};
use crate::git::{clone_or_open, copy_tree, dir_is_populated, fetch_all, resolve_ref};
use crate::lock::{LockedAddon, Lockfile};
use crate::manifest::{Manifest, validate};

pub fn clean() -> Result<()> {
    let cache_dir = cache_dir()?;
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }
    println!("cache cleared");
    Ok(())
}

pub fn install() -> Result<()> {
    let manifest_str =
        fs::read_to_string("gdep.toml").context("gdep.toml not found in current directory")?;
    let manifest: Manifest = toml::from_str(&manifest_str).context("failed to parse gdep.toml")?;

    let mut names: Vec<&String> = manifest.addons.keys().collect();
    names.sort();

    for name in &names {
        validate(name, &manifest.addons[*name])?;
    }

    let lock_str = match fs::read_to_string("gdep.lock") {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).context("failed to read gdep.lock"),
    };
    let mut lockfile: Lockfile = if lock_str.is_empty() {
        Lockfile::default()
    } else {
        toml::from_str(&lock_str).context("failed to parse gdep.lock")?
    };

    let cache_dir = cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    for name in names {
        let spec = &manifest.addons[name];
        let subdir = spec.subdir.as_deref().unwrap();
        let addon_dir = PathBuf::from("addons").join(name);
        let manifest_rev = spec.rev();

        // Lockfile entry is valid only if url, subdir, and ref all match the manifest
        let locked = lockfile
            .addons
            .get(name)
            .filter(|l| l.url == spec.url && l.subdir == subdir && l.rev == manifest_rev);

        let spinner = new_spinner(name);

        // Fast path: manifest unchanged and addon directory has files — no network needed
        if locked.is_some() && dir_is_populated(&addon_dir) {
            spinner.finish_and_clear();
            println!("{name}: up to date");
            continue;
        }
        let repo_dir = cache_dir.join(url_to_key(&spec.url));
        let repo = clone_or_open(&spec.url, &repo_dir, &spinner)?;

        let sha = if let Some(locked) = locked {
            let sha = locked.commit.clone();
            // Only fetch if the pinned SHA is not already in the local cache
            if repo.find_commit(git2::Oid::from_str(&sha)?).is_err() {
                spinner.set_message(format!("{name}: fetching"));
                fetch_all(&repo, &spec.url)?;
            }
            sha
        } else {
            spinner.set_message(format!("{name}: fetching"));
            fetch_all(&repo, &spec.url)?;
            resolve_ref(
                &repo,
                &spec.url,
                spec.tag.as_deref(),
                spec.branch.as_deref(),
                spec.commit.as_deref(),
            )?
        };

        spinner.set_message(format!("{name}: installing"));
        if addon_dir.exists() {
            fs::remove_dir_all(&addon_dir)?;
        }
        fs::create_dir_all(&addon_dir)?;

        let commit = repo
            .find_commit(git2::Oid::from_str(&sha)?)
            .with_context(|| format!("commit {sha} not found in {}", spec.url))?;
        let tree = commit.tree()?;
        let subtree_entry = tree
            .get_path(Path::new(subdir))
            .with_context(|| format!("subdir '{subdir}' not found at {sha} in {}", spec.url))?;
        let subtree = repo
            .find_tree(subtree_entry.id())
            .context("subdir is not a directory")?;

        copy_tree(&repo, &subtree, &addon_dir)?;
        spinner.finish_and_clear();
        println!("{name}: done");

        lockfile.addons.insert(
            name.clone(),
            LockedAddon {
                url: spec.url.clone(),
                rev: manifest_rev.to_string(),
                commit: sha,
                subdir: subdir.to_string(),
            },
        );
        // Write after each success so a partial run doesn't force re-fetching on the next run
        fs::write("gdep.lock", toml::to_string_pretty(&lockfile)?)?;
    }

    // Remove addons that are no longer in the manifest
    let to_remove: Vec<String> = lockfile
        .addons
        .keys()
        .filter(|name| !manifest.addons.contains_key(*name))
        .cloned()
        .collect();

    for name in to_remove {
        let addon_dir = PathBuf::from("addons").join(&name);
        if addon_dir.exists() {
            fs::remove_dir_all(&addon_dir)?;
            println!("{name}: removed");
        }
        lockfile.addons.remove(&name);
    }

    fs::write("gdep.lock", toml::to_string_pretty(&lockfile)?)?;

    Ok(())
}

fn new_spinner(name: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner} {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(format!("{name}: working"));
    pb
}
