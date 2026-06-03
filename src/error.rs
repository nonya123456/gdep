use thiserror::Error;

#[derive(Debug, Error)]
pub enum GdepError {
    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("lockfile error: {0}")]
    Lockfile(String),

    #[error("git error: {0}")]
    Git(#[from] git2::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("addon '{0}' not found in manifest")]
    AddonNotFound(String),

    #[error("could not resolve ref '{ref_}' for addon '{addon}'")]
    RefNotResolved { addon: String, ref_: String },

    #[error("cache directory unavailable: {0}")]
    CacheDir(String),

    #[error("no ref specified for addon '{0}': add tag, branch, or commit")]
    NoRef(String),
}
