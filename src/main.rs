use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "gdep", about = "Godot addon dependency manager")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Install all addons from gdep.toml
    Install,
}

#[derive(Deserialize)]
struct Manifest {
    addons: HashMap<String, AddonSpec>,
}

#[derive(Deserialize)]
struct AddonSpec {
    url: String,
    tag: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    subdir: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct Lockfile {
    #[serde(default)]
    addons: BTreeMap<String, LockedAddon>,
}

#[derive(Serialize, Deserialize, Clone)]
struct LockedAddon {
    url: String,
    commit: String,
    subdir: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install => install(),
    }
}

fn install() -> Result<()> {
    let manifest_str =
        fs::read_to_string("gdep.toml").context("gdep.toml not found in current directory")?;
    let manifest: Manifest = toml::from_str(&manifest_str).context("failed to parse gdep.toml")?;

    for (name, spec) in &manifest.addons {
        if spec.subdir.is_none() {
            bail!("addon '{name}': subdir is required");
        }
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
    }

    let lock_str = fs::read_to_string("gdep.lock").unwrap_or_default();
    let mut lockfile: Lockfile = if lock_str.is_empty() {
        Lockfile::default()
    } else {
        toml::from_str(&lock_str).context("failed to parse gdep.lock")?
    };

    let cache_dir = dirs::cache_dir()
        .context("could not determine cache directory")?
        .join("gdep");
    fs::create_dir_all(&cache_dir)?;

    let mut names: Vec<&String> = manifest.addons.keys().collect();
    names.sort();

    for name in names {
        let spec = &manifest.addons[name];
        let subdir = spec.subdir.as_deref().unwrap();
        let addon_dir = PathBuf::from("addons").join(name);
        let marker_path = addon_dir.join(".gdep");

        let lock_sha = lockfile.addons.get(name).map(|l| l.commit.clone());
        let marker_sha = if marker_path.exists() {
            fs::read_to_string(&marker_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        // Fast path: already installed at locked version — no network needed
        if let (Some(lock), Some(marker)) = (&lock_sha, &marker_sha)
            && lock == marker
        {
            println!("{name}: up to date");
            continue;
        }

        let repo_dir = cache_dir.join(url_to_key(&spec.url));
        let repo = open_or_clone(&spec.url, &repo_dir)?;

        let sha = if let Some(locked) = lock_sha {
            locked
        } else {
            resolve_ref(
                &repo,
                &spec.url,
                spec.tag.as_deref(),
                spec.branch.as_deref(),
                spec.commit.as_deref(),
            )?
        };

        if marker_sha.as_deref() == Some(sha.as_str()) {
            println!("{name}: up to date");
        } else {
            print!("{name}: installing... ");
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
            fs::write(&marker_path, &sha)?;
            println!("done");
        }

        lockfile.addons.insert(
            name.clone(),
            LockedAddon {
                url: spec.url.clone(),
                commit: sha,
                subdir: subdir.to_string(),
            },
        );
    }

    fs::write("gdep.lock", toml::to_string_pretty(&lockfile)?)?;

    Ok(())
}

fn url_to_key(url: &str) -> String {
    url.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}

fn open_or_clone(url: &str, repo_dir: &Path) -> Result<git2::Repository> {
    if repo_dir.exists() {
        let repo = git2::Repository::open_bare(repo_dir)
            .with_context(|| format!("failed to open cache for {url}"))?;
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
        drop(remote);
        Ok(repo)
    } else {
        println!("cloning {url}");
        git2::build::RepoBuilder::new()
            .bare(true)
            .clone(url, repo_dir)
            .with_context(|| format!("failed to clone {url}"))
    }
}

fn resolve_ref(
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

fn copy_tree(repo: &git2::Repository, tree: &git2::Tree, dest: &Path) -> Result<()> {
    for entry in tree.iter() {
        let name = entry.name().context("entry with non-UTF-8 name")?;
        let dest_path = dest.join(name);
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                fs::create_dir_all(&dest_path)?;
                let subtree = repo.find_tree(entry.id())?;
                copy_tree(repo, &subtree, &dest_path)?;
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
            _ => {}
        }
    }
    Ok(())
}
