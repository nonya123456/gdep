use anyhow::{Context, Result, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    /// Delete the local repo cache at ~/.cache/gdep/
    Clean,
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
    #[serde(default)]
    rev: String, // original tag/branch/commit from manifest
    commit: String, // resolved full SHA
    subdir: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install => install(),
        Command::Clean => clean(),
    }
}

fn cache_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("GDEP_CACHE_DIR") {
        return Ok(PathBuf::from(path));
    }
    dirs::cache_dir()
        .context("could not determine cache directory")
        .map(|p| p.join("gdep"))
}

fn clean() -> Result<()> {
    let cache_dir = cache_dir()?;
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }
    println!("cache cleared");
    Ok(())
}

fn install() -> Result<()> {
    let manifest_str =
        fs::read_to_string("gdep.toml").context("gdep.toml not found in current directory")?;
    let manifest: Manifest = toml::from_str(&manifest_str).context("failed to parse gdep.toml")?;

    for (name, spec) in &manifest.addons {
        if spec.url.is_empty() {
            bail!("addon '{name}': url must not be empty");
        }
        match spec.subdir.as_deref() {
            None => bail!("addon '{name}': subdir is required"),
            Some("") => bail!("addon '{name}': subdir must not be empty"),
            _ => {}
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
        let rev = spec
            .tag
            .as_deref()
            .or(spec.branch.as_deref())
            .or(spec.commit.as_deref())
            .unwrap();
        if rev.is_empty() {
            bail!("addon '{name}': tag/branch/commit value must not be empty");
        }
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

    let mut names: Vec<&String> = manifest.addons.keys().collect();
    names.sort();

    for name in names {
        let spec = &manifest.addons[name];
        let subdir = spec.subdir.as_deref().unwrap();
        let addon_dir = PathBuf::from("addons").join(name);

        let manifest_rev = spec
            .tag
            .as_deref()
            .or(spec.branch.as_deref())
            .or(spec.commit.as_deref())
            .unwrap();

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

// Percent-encode non-safe characters so different URLs always produce different keys.
// '/', ':', '@', etc. all encode to distinct %XX sequences — no collisions.
fn url_to_key(url: &str) -> String {
    let mut out = String::with_capacity(url.len() * 3);
    for b in url.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
                out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
            }
        }
    }
    out
}

// A directory is "populated" if it exists and contains at least one entry.
// Checking this instead of just exists() prevents a false "up to date" when
// the user deleted the directory contents but left the directory itself.
fn dir_is_populated(dir: &Path) -> bool {
    dir.read_dir().is_ok_and(|mut d| d.next().is_some())
}

// Opens an existing bare clone or creates a new one. Does NOT fetch — call
// fetch_all separately when you need the latest refs.
fn clone_or_open(url: &str, repo_dir: &Path, spinner: &ProgressBar) -> Result<git2::Repository> {
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

fn fetch_all(repo: &git2::Repository, url: &str) -> Result<()> {
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
