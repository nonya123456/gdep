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

fn git_capture(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::null())
        .output()
        .expect("git command failed to spawn");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn install_cmd(project: &Path, cache: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project)
        .env("GDEP_CACHE_DIR", cache)
        .arg("install")
        .assert()
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

#[cfg(unix)]
#[test]
fn test_install_preserves_symlink() {
    let repo_dir = tempfile::tempdir().unwrap();
    let path = repo_dir.path();
    let subdir = "addons/myaddon";

    fs::create_dir_all(path.join(subdir)).unwrap();
    fs::write(path.join(subdir).join("real.txt"), "hi").unwrap();
    std::os::unix::fs::symlink("real.txt", path.join(subdir).join("link.txt")).unwrap();

    git(path, &["init"]);
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "init"]);
    git(path, &["tag", "v1.0"]);
    let url = format!("file://{}", path.display());

    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"{subdir}\"\n"),
    )
    .unwrap();

    Command::cargo_bin("gdep")
        .unwrap()
        .current_dir(project.path())
        .env("GDEP_CACHE_DIR", cache.path())
        .arg("install")
        .assert()
        .success();

    let link = project.path().join("addons/myaddon/link.txt");
    let meta = fs::symlink_metadata(&link).unwrap();
    assert!(
        meta.file_type().is_symlink(),
        "link.txt should be installed as a symlink"
    );
    assert_eq!(fs::read_link(&link).unwrap().to_str().unwrap(), "real.txt");
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

#[test]
fn test_install_by_branch() {
    let (repo, url) = make_local_repo("addons/myaddon");
    git(repo.path(), &["branch", "stable"]);
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        format!(
            "[addons.myaddon]\nurl = \"{url}\"\nbranch = \"stable\"\nsubdir = \"addons/myaddon\"\n"
        ),
    )
    .unwrap();

    install_cmd(project.path(), cache.path()).success();
    assert!(project.path().join("addons/myaddon/plugin.cfg").exists());
}

#[test]
fn test_install_by_commit() {
    let (repo, url) = make_local_repo("addons/myaddon");
    let sha = git_capture(repo.path(), &["rev-parse", "HEAD"]);
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        format!(
            "[addons.myaddon]\nurl = \"{url}\"\ncommit = \"{sha}\"\nsubdir = \"addons/myaddon\"\n"
        ),
    )
    .unwrap();

    install_cmd(project.path(), cache.path()).success();
    assert!(project.path().join("addons/myaddon/plugin.cfg").exists());
}

#[test]
fn test_install_reresolves_on_tag_change() {
    // A repo with two tags pointing at different content.
    let repo = tempfile::tempdir().unwrap();
    let rp = repo.path();
    let subdir = "addons/myaddon";
    fs::create_dir_all(rp.join(subdir)).unwrap();
    fs::write(rp.join(subdir).join("v.txt"), "one").unwrap();
    git(rp, &["init"]);
    git(rp, &["add", "."]);
    git(rp, &["commit", "-m", "one"]);
    git(rp, &["tag", "v1.0"]);
    fs::write(rp.join(subdir).join("v.txt"), "two").unwrap();
    git(rp, &["add", "."]);
    git(rp, &["commit", "-m", "two"]);
    git(rp, &["tag", "v2.0"]);
    let url = format!("file://{}", rp.display());

    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let pp = project.path();
    let manifest = |tag: &str| {
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"{tag}\"\nsubdir = \"{subdir}\"\n")
    };

    fs::write(pp.join("gdep.toml"), manifest("v1.0")).unwrap();
    install_cmd(pp, cache.path()).success();
    assert_eq!(
        fs::read_to_string(pp.join("addons/myaddon/v.txt")).unwrap(),
        "one"
    );
    let lock1 = fs::read_to_string(pp.join("gdep.lock")).unwrap();

    // Bump the tag: content and the locked commit should both change.
    fs::write(pp.join("gdep.toml"), manifest("v2.0")).unwrap();
    install_cmd(pp, cache.path()).success();
    assert_eq!(
        fs::read_to_string(pp.join("addons/myaddon/v.txt")).unwrap(),
        "two"
    );
    let lock2 = fs::read_to_string(pp.join("gdep.lock")).unwrap();
    assert_ne!(lock1, lock2, "lock should record the new commit");
}

#[test]
fn test_install_copies_nested_dirs() {
    let repo = tempfile::tempdir().unwrap();
    let rp = repo.path();
    let subdir = "addons/myaddon";
    fs::create_dir_all(rp.join(subdir).join("scripts")).unwrap();
    fs::write(rp.join(subdir).join("plugin.cfg"), "x").unwrap();
    fs::write(
        rp.join(subdir).join("scripts").join("util.gd"),
        "func f(): pass\n",
    )
    .unwrap();
    git(rp, &["init"]);
    git(rp, &["add", "."]);
    git(rp, &["commit", "-m", "init"]);
    git(rp, &["tag", "v1.0"]);
    let url = format!("file://{}", rp.display());

    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"{subdir}\"\n"),
    )
    .unwrap();

    install_cmd(project.path(), cache.path()).success();
    assert!(
        project
            .path()
            .join("addons/myaddon/scripts/util.gd")
            .exists()
    );
}

#[test]
fn test_install_recopies_emptied_dir() {
    let (_repo, url) = make_local_repo("addons/myaddon");
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let pp = project.path();

    fs::write(
        pp.join("gdep.toml"),
        format!("[addons.myaddon]\nurl = \"{url}\"\ntag = \"v1.0\"\nsubdir = \"addons/myaddon\"\n"),
    )
    .unwrap();

    install_cmd(pp, cache.path()).success();

    // Delete the contents but leave the directory: the lock still matches, so
    // only dir_is_populated() forces a re-copy here.
    fs::remove_file(pp.join("addons/myaddon/plugin.cfg")).unwrap();
    assert!(pp.join("addons/myaddon").exists());

    install_cmd(pp, cache.path()).success();
    assert!(pp.join("addons/myaddon/plugin.cfg").exists());
}

#[test]
fn test_install_rejects_two_refs() {
    let project = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    fs::write(
        project.path().join("gdep.toml"),
        "[addons.myaddon]\nurl = \"file:///tmp/fake\"\ntag = \"v1.0\"\nbranch = \"main\"\nsubdir = \"addons/x\"\n",
    )
    .unwrap();

    install_cmd(project.path(), cache.path())
        .failure()
        .stderr(predicate::str::contains("only one of"));
}
