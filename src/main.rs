mod cache;
mod cli;
mod error;
mod fetcher;
mod installer;
mod lockfile;
mod manifest;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use lockfile::{LockedAddon, Lockfile};
use manifest::{AddonEntry, Manifest};
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = std::env::current_dir()?;

    match cli.command {
        Commands::Init => cmd_init(&project_dir),
        Commands::Add { url, name, tag, branch, commit, subdir } => {
            cmd_add(&project_dir, url, name, tag, branch, commit, subdir)
        }
        Commands::Install => cmd_install(&project_dir),
        Commands::Update { name } => cmd_update(&project_dir, name.as_deref()),
        Commands::Remove { name } => cmd_remove(&project_dir, &name),
        Commands::List => cmd_list(&project_dir),
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_init(dir: &PathBuf) -> Result<()> {
    Manifest::init(dir).context("init failed")?;
    println!("Created gdep.toml");
    Ok(())
}

fn cmd_add(
    dir: &PathBuf,
    url: String,
    name: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    subdir: Option<String>,
) -> Result<()> {
    let mut manifest = Manifest::load(dir).context("load manifest")?;

    let addon_name = name.unwrap_or_else(|| derive_name(&url));

    if tag.is_none() && branch.is_none() && commit.is_none() {
        anyhow::bail!(
            "specify at least one of --tag, --branch, or --commit.\n\
             Example: gdep add {url} --branch main"
        );
    }

    let entry = AddonEntry {
        git: url,
        tag,
        branch,
        commit,
        subdirectory: subdir,
    };

    manifest.addons.insert(addon_name.clone(), entry);
    manifest.save(dir).context("save manifest")?;

    println!("Added '{addon_name}' to gdep.toml  (run `gdep install` to fetch)");
    Ok(())
}

fn cmd_install(dir: &PathBuf) -> Result<()> {
    let manifest = Manifest::load(dir).context("load manifest")?;

    // Load existing lockfile or start fresh.
    let mut lock = Lockfile::load(dir).unwrap_or_default();

    for (name, entry) in &manifest.addons {
        print!("  {name} ({}) … ", entry.ref_display());

        // If lockfile already has a pinned commit for a tag/commit entry,
        // trust it and skip network (unless it's missing from cache).
        let existing = lock.find(name).map(|l| l.commit.clone());

        let commit = if entry.branch.is_none() {
            if let Some(ref c) = existing {
                // Verify cache has it; re-fetch only on cache miss.
                match fetcher::verify_cached(name, entry, c) {
                    Ok(_) => {
                        println!("cached ({c:.8})");
                        c.clone()
                    }
                    Err(_) => {
                        let resolved = fetcher::resolve_and_fetch(name, entry)?;
                        println!("{resolved:.8}");
                        resolved
                    }
                }
            } else {
                let resolved = fetcher::resolve_and_fetch(name, entry)?;
                println!("{resolved:.8}");
                resolved
            }
        } else {
            // Branch-pinned: always fetch, but only update lockfile if
            // no entry yet (update is done via `gdep update`).
            if existing.is_none() {
                let resolved = fetcher::resolve_and_fetch(name, entry)?;
                println!("{resolved:.8}");
                resolved
            } else {
                let c = existing.unwrap();
                match fetcher::verify_cached(name, entry, &c) {
                    Ok(_) => {
                        println!("cached ({c:.8})");
                        c.clone()
                    }
                    Err(_) => {
                        let resolved = fetcher::resolve_and_fetch(name, entry)?;
                        println!("{resolved:.8}");
                        resolved
                    }
                }
            }
        };

        let locked = LockedAddon {
            name: name.clone(),
            git: entry.git.clone(),
            commit,
            subdirectory: entry.subdirectory.clone(),
        };
        lock.upsert(locked.clone());

        installer::install_addon(&locked, dir)?;
    }

    lock.save(dir).context("save lockfile")?;
    println!("Done. gdep.lock updated.");
    Ok(())
}

fn cmd_update(dir: &PathBuf, name: Option<&str>) -> Result<()> {
    let manifest = Manifest::load(dir).context("load manifest")?;
    let mut lock = Lockfile::load(dir).unwrap_or_default();

    let targets: Vec<(&String, &AddonEntry)> = manifest
        .addons
        .iter()
        .filter(|(n, e)| {
            e.branch.is_some() && name.map_or(true, |target| target == n.as_str())
        })
        .collect();

    if targets.is_empty() {
        if let Some(n) = name {
            anyhow::bail!("'{n}' not found or not branch-pinned");
        } else {
            println!("No branch-pinned addons to update.");
            return Ok(());
        }
    }

    for (n, entry) in targets {
        print!("  Updating {n} ({}) … ", entry.ref_display());
        let commit = fetcher::resolve_and_fetch(n, entry)?;
        println!("{commit:.8}");

        let locked = LockedAddon {
            name: n.clone(),
            git: entry.git.clone(),
            commit,
            subdirectory: entry.subdirectory.clone(),
        };
        lock.upsert(locked.clone());
        installer::install_addon(&locked, dir)?;
    }

    lock.save(dir).context("save lockfile")?;
    println!("Done. gdep.lock updated.");
    Ok(())
}

fn cmd_remove(dir: &PathBuf, name: &str) -> Result<()> {
    let mut manifest = Manifest::load(dir).context("load manifest")?;
    if !manifest.addons.contains_key(name) {
        anyhow::bail!("addon '{name}' not in gdep.toml");
    }
    manifest.addons.remove(name);
    manifest.save(dir).context("save manifest")?;

    let mut lock = Lockfile::load(dir).unwrap_or_default();
    lock.remove(name);
    lock.save(dir).context("save lockfile")?;

    installer::remove_addon(name, dir)?;
    println!("Removed '{name}'.");
    Ok(())
}

fn cmd_list(dir: &PathBuf) -> Result<()> {
    let lock = Lockfile::load(dir).context("load lockfile — run `gdep install` first")?;

    if lock.addon.is_empty() {
        println!("No addons installed.");
        return Ok(());
    }

    println!("{:<30} {:<10} {}", "name", "commit", "source");
    println!("{}", "-".repeat(80));
    for a in &lock.addon {
        let short = &a.commit[..a.commit.len().min(8)];
        println!("{:<30} {:<10} {}", a.name, short, a.git);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn derive_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("addon")
        .trim_end_matches(".git")
        .to_string()
}
