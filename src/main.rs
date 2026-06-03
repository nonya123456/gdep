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
use std::path::Path;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = std::env::current_dir()?;

    match cli.command {
        Commands::Init => cmd_init(&project_dir),
        Commands::Add {
            url,
            name,
            tag,
            branch,
            commit,
            subdir,
        } => cmd_add(&project_dir, url, name, tag, branch, commit, subdir),
        Commands::Install => cmd_install(&project_dir),
        Commands::Update { name } => cmd_update(&project_dir, name.as_deref()),
        Commands::Remove { name } => cmd_remove(&project_dir, &name),
        Commands::List => cmd_list(&project_dir),
    }
}

fn cmd_init(dir: &Path) -> Result<()> {
    Manifest::init(dir).context("init failed")?;
    println!("Created gdep.toml");
    Ok(())
}

fn cmd_add(
    dir: &Path,
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

    manifest.addons.insert(
        addon_name.clone(),
        AddonEntry {
            git: url,
            tag,
            branch,
            commit,
            subdirectory: subdir,
        },
    );
    manifest.save(dir).context("save manifest")?;

    println!("Added '{addon_name}' to gdep.toml  (run `gdep install` to fetch)");
    Ok(())
}

fn cmd_install(dir: &Path) -> Result<()> {
    let manifest = Manifest::load(dir).context("load manifest")?;
    let mut lock = Lockfile::load(dir).unwrap_or_default();

    for (name, entry) in &manifest.addons {
        print!("  {name} ({}) … ", entry.ref_display());

        let existing = lock.find(name).map(|l| l.commit.clone());

        let commit = if entry.branch.is_none() {
            if let Some(ref c) = existing {
                match fetcher::verify_cached(name, entry, c) {
                    Ok(_) => {
                        println!("cached ({c:.8})");
                        c.clone()
                    }
                    Err(_) => {
                        let r = fetcher::resolve_and_fetch(name, entry)?;
                        println!("{r:.8}");
                        r
                    }
                }
            } else {
                let r = fetcher::resolve_and_fetch(name, entry)?;
                println!("{r:.8}");
                r
            }
        } else {
            // Branch-pinned: lock commit on first install; use `gdep update` to advance.
            if let Some(c) = existing {
                match fetcher::verify_cached(name, entry, &c) {
                    Ok(_) => {
                        println!("cached ({c:.8})");
                        c
                    }
                    Err(_) => {
                        let r = fetcher::resolve_and_fetch(name, entry)?;
                        println!("{r:.8}");
                        r
                    }
                }
            } else {
                let r = fetcher::resolve_and_fetch(name, entry)?;
                println!("{r:.8}");
                r
            }
        };

        let locked = LockedAddon {
            name: name.clone(),
            git: entry.git.clone(),
            commit,
            subdirectory: entry.subdirectory.clone(),
        };
        let install_result = installer::install_addon(&locked, dir);
        if install_result.is_ok() {
            lock.upsert(locked);
        }
        lock.save(dir).context("save lockfile")?;
        install_result?;
    }

    println!("Done. gdep.lock updated.");
    Ok(())
}

fn cmd_update(dir: &Path, name: Option<&str>) -> Result<()> {
    let manifest = Manifest::load(dir).context("load manifest")?;
    let mut lock = Lockfile::load(dir).unwrap_or_default();

    let targets: Vec<(&String, &AddonEntry)> = manifest
        .addons
        .iter()
        .filter(|(n, e)| e.branch.is_some() && name.is_none_or(|t| t == n.as_str()))
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
        let install_result = installer::install_addon(&locked, dir);
        if install_result.is_ok() {
            lock.upsert(locked);
        }
        lock.save(dir).context("save lockfile")?;
        install_result?;
    }

    println!("Done. gdep.lock updated.");
    Ok(())
}

fn cmd_remove(dir: &Path, name: &str) -> Result<()> {
    let mut manifest = Manifest::load(dir).context("load manifest")?;
    if !manifest.addons.contains_key(name) {
        anyhow::bail!("addon '{name}' not in gdep.toml");
    }
    manifest.addons.remove(name);
    manifest.save(dir).context("save manifest")?;

    let mut lock = Lockfile::load(dir).unwrap_or_default();
    let locked = lock.find(name).cloned();
    lock.remove(name);
    lock.save(dir).context("save lockfile")?;

    if let Some(locked) = locked {
        installer::remove_addon(&locked, dir)?;
    } else {
        let dest = dir.join("addons").join(name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
    }
    println!("Removed '{name}'.");
    Ok(())
}

fn cmd_list(dir: &Path) -> Result<()> {
    let lock = Lockfile::load(dir).context("load lockfile — run `gdep install` first")?;

    if lock.addon.is_empty() {
        println!("No addons installed.");
        return Ok(());
    }

    println!("{:<30} {:<10} source", "name", "commit");
    println!("{}", "-".repeat(80));
    for a in &lock.addon {
        let short = &a.commit[..a.commit.len().min(8)];
        println!("{:<30} {:<10} {}", a.name, short, a.git);
    }
    Ok(())
}

fn derive_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("addon")
        .trim_end_matches(".git")
        .to_string()
}
