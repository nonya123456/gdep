use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn cache_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("GDEP_CACHE_DIR") {
        return Ok(PathBuf::from(path));
    }
    dirs::cache_dir()
        .context("could not determine cache directory")
        .map(|p| p.join("gdep"))
}

// Percent-encode non-safe characters so different URLs map to different keys.
// '/', ':', '@', etc. all encode to distinct %XX sequences. Note that on
// case-insensitive filesystems (macOS, Windows) two URLs differing only in
// case still collide — the encoding is exact but the filesystem is not.
pub fn url_to_key(url: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_unsafe_chars() {
        assert_eq!(
            url_to_key("https://github.com/a/b"),
            "https%3a%2f%2fgithub.com%2fa%2fb"
        );
    }

    #[test]
    fn distinct_urls_distinct_keys() {
        assert_ne!(
            url_to_key("https://github.com/a/b"),
            url_to_key("https://github.com/a-b")
        );
    }

    #[test]
    fn key_has_no_path_separators() {
        let key = url_to_key("https://github.com/a/b");
        assert!(!key.contains('/'));
        assert!(!key.contains('\\'));
    }
}
