use crate::error::GdepError;
use dirs::cache_dir;
use std::path::PathBuf;

pub fn repo_cache_path(url: &str) -> Result<PathBuf, GdepError> {
    let base = cache_dir()
        .ok_or_else(|| GdepError::CacheDir("cannot determine cache dir".into()))?
        .join("gdep");
    std::fs::create_dir_all(&base).map_err(GdepError::Io)?;
    Ok(base.join(url_hash(url)))
}

fn url_hash(url: &str) -> String {
    // djb2 — good enough for a local cache key
    let mut h: u64 = 5381;
    for b in url.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    format!("{h:016x}")
}
