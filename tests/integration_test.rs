use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git command failed to spawn");
    assert!(status.success(), "git {args:?} failed");
}

fn make_local_repo(subdir: &str) -> (TempDir, String) {
    let repo_dir = tempfile::tempdir().unwrap();
    let path = repo_dir.path();

    fs::create_dir_all(path.join(subdir)).unwrap();
    fs::write(
        path.join(subdir).join("plugin.cfg"),
        "[plugin]\nname=\"test\"\n",
    )
    .unwrap();

    git(path, &["init"]);
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "init"]);
    git(path, &["tag", "v1.0"]);

    let url = format!("file://{}", path.display());
    (repo_dir, url)
}

#[test]
fn test_clean_no_cache() {
    let cache = tempfile::tempdir().unwrap();
    let cache_path = cache.path().join("gdep");

    Command::cargo_bin("gdep")
        .unwrap()
        .env("GDEP_CACHE_DIR", &cache_path)
        .arg("clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("cache cleared"));
}

#[test]
fn test_clean_existing_cache() {
    let cache = tempfile::tempdir().unwrap();
    let cache_path = cache.path().join("gdep");
    fs::create_dir_all(&cache_path).unwrap();
    fs::write(cache_path.join("dummy"), "data").unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .env("GDEP_CACHE_DIR", &cache_path)
        .arg("clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("cache cleared"));

    assert!(!cache_path.exists());
}

#[test]
fn test_install_fresh() {
    let (_repo, url) = make_local_repo("addons/myaddon");
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"addons/myaddon\"\n"),
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success();

    assert!(project.path().join("addons/myaddon/plugin.cfg").exists());
    assert!(project.path().join("gdep.lock").exists());
}

#[test]
fn test_install_up_to_date() {
    let (_repo, url) = make_local_repo("addons/myaddon");
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"addons/myaddon\"\n"),
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("myaddon: up to date"));
}

#[test]
fn test_install_missing_subdir() {
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        "[addons.myaddon]\nurl = \"file:///tmp/fake\"\ntag = \"v1.0\"\n",
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicate::str::contains("subdir is required"));
}

#[test]
fn test_install_rejects_traversal_name() {
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        "[addons.\"../escape\"]\nurl = \"file:///tmp/fake\"\ntag = \"v1.0\"\nsubdir = \"addons/x\"\n",
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicate::str::contains("must not contain"));

    // Nothing was created outside the project directory.
    assert!(!project.path().parent().unwrap().join("escape").exists());
}

#[test]
fn test_install_removes_stale() {
    let (_repo, url) = make_local_repo("addons/myaddon");
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"addons/myaddon\"\n"),
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success();

    assert!(project.path().join("addons/myaddon").exists());

    // Remove the addon from the manifest
    fs::write(project.path().join("gdep.toml"), "[addons]\n").unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("myaddon: removed"));

    assert!(!project.path().join("addons/myaddon").exists());
}
