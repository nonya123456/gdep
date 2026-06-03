use crate::error::GdepError;
use dirs::cache_dir;
use std::path::PathBuf;

pub fn repo_cache_path(url: &str) -> Result<PathBuf, GdepError> {
    let base = if let Ok(dir) = std::env::var("GDEP_CACHE_DIR") {
        PathBuf::from(dir)
    } else {
        cache_dir()
            .ok_or_else(|| GdepError::CacheDir("cannot determine cache dir".into()))?
            .join("gdep")
    };
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

#[cfg(test)]
mod tests {
    use super::url_hash;

    #[test]
    fn url_hash_is_deterministic() {
        assert_eq!(
            url_hash("https://github.com/foo/bar"),
            url_hash("https://github.com/foo/bar")
        );
    }

    #[test]
    fn url_hash_differs_for_different_inputs() {
        assert_ne!(
            url_hash("https://github.com/foo/bar"),
            url_hash("https://github.com/foo/baz")
        );
    }

    #[test]
    fn url_hash_is_16_hex_chars() {
        let h = url_hash("anything");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn url_hash_empty_string_is_djb2_initial_value() {
        // djb2 starts at 5381 with no bytes processed
        assert_eq!(url_hash(""), format!("{:016x}", 5381u64));
    }
}
