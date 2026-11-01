//! Recently opened files, remembered between runs in `~/.config/anvil/recent`
//! (one absolute path per line, newest first).

use std::fs;
use std::path::{Path, PathBuf};

/// How many paths are kept on disk.
pub const MAX_STORED: usize = 20;

fn store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("anvil").join("recent")
}

/// Absolute, symlink-free form of an existing path, so `anvil a.rs` and
/// `anvil ./a.rs` count as the same file. `None` if it doesn't exist.
pub fn canonical(p: &Path) -> Option<PathBuf> {
    fs::canonicalize(p).ok()
}

/// Move `p` to the front, drop duplicates, cap the length.
pub fn push(list: &mut Vec<PathBuf>, p: PathBuf) {
    list.retain(|x| *x != p);
    list.insert(0, p);
    list.truncate(MAX_STORED);
}

/// Parse the stored text (blank lines ignored, duplicates dropped).
fn parse(text: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let p = PathBuf::from(line);
        if !out.contains(&p) { out.push(p); }
    }
    out.truncate(MAX_STORED);
    out
}

/// Load the saved list; files that no longer exist are skipped.
pub fn load() -> Vec<PathBuf> {
    let text = fs::read_to_string(store_path()).unwrap_or_default();
    parse(&text).into_iter().filter(|p| p.exists()).collect()
}

/// Save the list (best effort: failing to write must never disturb editing).
pub fn save(list: &[PathBuf]) {
    let path = store_path();
    if let Some(dir) = path.parent() { let _ = fs::create_dir_all(dir); }
    let text: String = list.iter().map(|p| format!("{}\n", p.display())).collect();
    let _ = fs::write(path, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf { PathBuf::from(s) }

    #[test]
    fn push_moves_to_front_without_duplicates() {
        let mut l = vec![p("/a"), p("/b"), p("/c")];
        push(&mut l, p("/c"));
        assert_eq!(l, [p("/c"), p("/a"), p("/b")]);
        push(&mut l, p("/d"));
        assert_eq!(l, [p("/d"), p("/c"), p("/a"), p("/b")]);
    }

    #[test]
    fn push_caps_the_list() {
        let mut l = Vec::new();
        for i in 0..(MAX_STORED + 5) { push(&mut l, p(&format!("/f{}", i))); }
        assert_eq!(l.len(), MAX_STORED);
        assert_eq!(l[0], p(&format!("/f{}", MAX_STORED + 4)));
    }

    #[test]
    fn parse_ignores_blanks_and_duplicates() {
        assert_eq!(parse("/a\n\n  /b  \n/a\n"), [p("/a"), p("/b")]);
        assert!(parse("").is_empty());
    }
}
