use git2::{IndexAddOption, Repository, Signature, Time};
use tempfile::{tempdir, TempDir};

pub static CACHE_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct RepoFixture {
    pub dir: TempDir,
    pub commit_sha: String,
    pub default_branch: String,
}

impl RepoFixture {
    pub fn url(&self) -> String {
        self.dir.path().to_str().unwrap().to_string()
    }
}

/// Non-bare repo with `addons/<addon_name>/plugin.gd` and a lightweight tag.
pub fn make_addons_repo(addon_name: &str, tag: &str) -> RepoFixture {
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    configure(&repo);
    let addon_dir = dir.path().join("addons").join(addon_name);
    std::fs::create_dir_all(&addon_dir).unwrap();
    std::fs::write(addon_dir.join("plugin.gd"), b"# plugin").unwrap();
    let (sha, branch) = commit_all(&repo, "init");
    tag_commit(&repo, tag, &sha);
    RepoFixture { dir, commit_sha: sha, default_branch: branch }
}

/// Non-bare repo with `plugin.gd` at root (no `addons/` dir).
pub fn make_root_repo(tag: &str) -> RepoFixture {
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    configure(&repo);
    std::fs::write(dir.path().join("plugin.gd"), b"# plugin").unwrap();
    let (sha, branch) = commit_all(&repo, "init");
    tag_commit(&repo, tag, &sha);
    RepoFixture { dir, commit_sha: sha, default_branch: branch }
}

/// Non-bare repo with `<subdir>/plugin.gd` (for subdirectory strategy testing).
pub fn make_subdir_repo(subdir: &str, tag: &str) -> RepoFixture {
    let dir = tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    configure(&repo);
    std::fs::create_dir_all(dir.path().join(subdir)).unwrap();
    std::fs::write(dir.path().join(subdir).join("plugin.gd"), b"# plugin").unwrap();
    let (sha, branch) = commit_all(&repo, "init");
    tag_commit(&repo, tag, &sha);
    RepoFixture { dir, commit_sha: sha, default_branch: branch }
}

pub fn set_test_cache(cache_dir: &TempDir) {
    // SAFETY: tests using this must hold CACHE_MUTEX to avoid concurrent env mutation.
    unsafe {
        std::env::set_var("GDEP_CACHE_DIR", cache_dir.path().to_str().unwrap());
    }
}

pub fn unset_test_cache() {
    unsafe {
        std::env::remove_var("GDEP_CACHE_DIR");
    }
}

fn configure(repo: &Repository) {
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "test").unwrap();
    cfg.set_str("user.email", "test@test.invalid").unwrap();
}

fn commit_all(repo: &Repository, msg: &str) -> (String, String) {
    let sig = Signature::new("test", "test@test.invalid", &Time::new(0, 0)).unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
    idx.write().unwrap();
    let tree_oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[]).unwrap();
    let branch = repo.head().unwrap().shorthand().unwrap().to_string();
    (oid.to_string(), branch)
}

fn tag_commit(repo: &Repository, tag: &str, sha: &str) {
    let oid = git2::Oid::from_str(sha).unwrap();
    let obj = repo.find_object(oid, None).unwrap();
    repo.tag_lightweight(tag, &obj, false).unwrap();
}
