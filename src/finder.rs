use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ignore::WalkBuilder;

/// How many ranked results are kept for the list.
pub const MAX_RESULTS: usize = 500;
/// Safety cap on how many files are indexed (searching all of `$HOME`).
const MAX_INDEXED: usize = 500_000;
/// Live grep stops after this many matching lines.
const MAX_GREP: usize = 500;
/// Files bigger than this are not searched by live grep.
const MAX_GREP_FILE: u64 = 2_000_000;
/// Folders that are never worth searching: huge, generated or internal.
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "__pycache__", ".cache", ".npm", ".cargo", ".rustup",
    ".gradle", ".m2", ".venv", "venv", ".mozilla", ".steam", ".wine", ".var",
    "snap", ".Trash", "Trash",
];

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum FinderKind {
    File,
    Grep,
}

pub struct GrepHit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

/// One indexed file. `rel` is lowercase and relative to the search root, so
/// matching never sees the (identical) prefix like `/home/user/`.
struct Entry {
    path: PathBuf,
    rel: String,
    name_at: usize,
}

fn make_entry(root: &Path, path: PathBuf) -> Entry {
    let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_lowercase();
    let name_at = rel.rfind('/').map_or(0, |i| i + 1);
    Entry { path, rel, name_at }
}

/// A running live-grep search. Dropping it cancels the search, so typing
/// another letter stops the old walk right away.
struct GrepJob {
    rx: Receiver<Vec<GrepHit>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for GrepJob {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

pub struct Finder {
    pub kind: FinderKind,
    pub query: String,
    pub visible: bool,
    pub selected: usize,
    /// First visible row of the list (scrolling).
    pub offset: usize,
    /// Height of the list in rows, as of the last draw (for PageUp/PageDown).
    pub view_h: usize,
    /// Folder being searched; result paths are shown relative to it.
    pub root: PathBuf,
    pub file_results: Vec<PathBuf>,
    pub grep_results: Vec<GrepHit>,
    entries: Vec<Entry>,
    rx: Option<Receiver<Vec<PathBuf>>>,
    grep_job: Option<GrepJob>,
    dirty: bool,
    last_filter: Instant,
    opened_at: Instant,
}

impl Finder {
    pub fn new() -> Self {
        Self {
            kind: FinderKind::File,
            query: String::new(),
            visible: false,
            selected: 0,
            offset: 0,
            view_h: 10,
            root: PathBuf::new(),
            file_results: Vec::new(),
            grep_results: Vec::new(),
            entries: Vec::new(),
            rx: None,
            grep_job: None,
            dirty: false,
            last_filter: Instant::now(),
            opened_at: Instant::now(),
        }
    }

    /// Open the file finder. Returns immediately; files stream in from a
    /// background thread (see `poll`).
    pub fn open_files(&mut self, root: &Path) {
        self.kind = FinderKind::File;
        self.query.clear();
        self.selected = 0;
        self.offset = 0;
        self.visible = true;
        self.root = root.to_path_buf();
        self.entries.clear();
        self.file_results.clear();
        self.grep_job = None;
        self.rx = Some(spawn_index(root.to_path_buf()));
        self.dirty = false;
        self.last_filter = Instant::now();
        self.opened_at = Instant::now();
    }

    pub fn open_grep(&mut self, root: &Path) {
        self.kind = FinderKind::Grep;
        self.query.clear();
        self.selected = 0;
        self.offset = 0;
        self.visible = true;
        self.root = root.to_path_buf();
        self.rx = None;
        self.grep_job = None;
        self.grep_results.clear();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.rx = None; // the indexing thread notices and stops
        self.grep_job = None; // dropping the job cancels the search
        self.grep_results.clear();
        self.entries = Vec::new();
        self.file_results.clear();
    }

    /// Still scanning folders?
    pub fn indexing(&self) -> bool { self.rx.is_some() }

    /// A live-grep search is still running.
    pub fn searching(&self) -> bool { self.grep_job.is_some() }

    /// Number of files indexed so far.
    pub fn indexed(&self) -> usize { self.entries.len() }

    /// Spinner frame for the "indexing" indicator.
    pub fn spinner(&self) -> char {
        const F: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        F[(self.opened_at.elapsed().as_millis() / 90) as usize % F.len()]
    }

    /// Take newly indexed files from the background thread and refresh the
    /// results. Cheap when nothing changed; call once per frame.
    pub fn poll(&mut self) {
        if self.kind == FinderKind::Grep { self.poll_grep(); return; }
        if self.rx.is_none() { return; }
        let mut got = false;
        let mut finished = false;
        if let Some(rx) = &self.rx {
            for _ in 0..32 {
                match rx.try_recv() {
                    Ok(batch) => {
                        for p in batch { self.entries.push(make_entry(&self.root, p)); }
                        got = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => { finished = true; break; }
                }
            }
        }
        if finished { self.rx = None; }
        if got || finished { self.dirty = true; }
        if self.dirty && self.kind == FinderKind::File {
            let due = finished || self.last_filter.elapsed() >= Duration::from_millis(80);
            // don't reshuffle the list under someone who is already browsing it
            let browsing = self.selected > 0 && !finished;
            if due && !browsing {
                self.refilter();
                self.dirty = false;
                self.last_filter = Instant::now();
            }
        }
    }

    /// Called after the query was edited: new search, back to the top.
    pub fn update_files(&mut self) {
        self.selected = 0;
        self.offset = 0;
        self.refilter();
    }

    fn refilter(&mut self) {
        let tokens: Vec<String> = self.query.to_lowercase().split_whitespace().map(String::from).collect();
        self.file_results = rank(&self.entries, &tokens, MAX_RESULTS);
        if self.selected >= self.file_results.len() {
            self.selected = self.file_results.len().saturating_sub(1);
        }
    }

    /// Start a new live-grep search for the current query. Returns at once;
    /// hits stream in through `poll`. Any search already running is cancelled.
    pub fn update_grep(&mut self, root: &Path) {
        self.grep_job = None;
        self.grep_results.clear();
        self.selected = 0;
        self.offset = 0;
        if self.query.is_empty() {
            return;
        }
        self.grep_job = Some(spawn_grep(root.to_path_buf(), self.query.to_lowercase()));
    }

    fn poll_grep(&mut self) {
        let mut finished = false;
        if let Some(job) = &self.grep_job {
            for _ in 0..32 {
                match job.rx.try_recv() {
                    Ok(hits) => self.grep_results.extend(hits),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => { finished = true; break; }
                }
            }
        }
        if finished { self.grep_job = None; }
    }

    pub fn len(&self) -> usize {
        match self.kind {
            FinderKind::File => self.file_results.len(),
            FinderKind::Grep => self.grep_results.len(),
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.len() {
            self.selected += 1;
        }
    }

    pub fn move_by(&mut self, delta: isize) {
        let max = self.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
    }

    pub fn page_up(&mut self) { self.move_by(-(self.view_h.max(1) as isize)); }
    pub fn page_down(&mut self) { self.move_by(self.view_h.max(1) as isize); }
    pub fn first(&mut self) { self.selected = 0; }
    pub fn last(&mut self) { self.selected = self.len().saturating_sub(1); }

    /// Keep the selected row inside the visible window of `height` rows.
    pub fn clamp_scroll(&mut self, height: usize) {
        self.view_h = height;
        if height == 0 { return; }
        let total = self.len();
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
        self.offset = self.offset.min(total.saturating_sub(height));
    }
}

// ============================================================
// Indexing (background thread)
// ============================================================

/// Folder filter: skip junk directories. `target/` is only skipped next to a
/// `Cargo.toml`, so a folder that happens to be called `target` is still found.
fn keep(e: &ignore::DirEntry) -> bool {
    if e.depth() == 0 { return true; }
    if e.file_type().map_or(false, |t| t.is_dir()) {
        let name = e.file_name().to_string_lossy();
        if SKIP_DIRS.contains(&name.as_ref()) { return false; }
        if name == "target" && e.path().parent().map_or(false, |p| p.join("Cargo.toml").exists()) {
            return false;
        }
    }
    true
}

/// Walk `root` on a thread, sending files in batches. The walk stops by
/// itself when the receiver is dropped (finder closed).
fn spawn_index(root: PathBuf) -> Receiver<Vec<PathBuf>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut batch: Vec<PathBuf> = Vec::with_capacity(512);
        let mut total = 0usize;
        let mut last = Instant::now();
        let walker = WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .filter_entry(keep)
            .build();
        for e in walker.flatten() {
            if !e.file_type().map_or(false, |t| t.is_file()) { continue; }
            batch.push(e.into_path());
            total += 1;
            if batch.len() >= 512 || last.elapsed() >= Duration::from_millis(40) {
                if tx.send(std::mem::take(&mut batch)).is_err() { return; }
                last = Instant::now();
            }
            if total >= MAX_INDEXED { break; }
        }
        let _ = tx.send(batch);
    });
    rx
}

/// Search file contents for `q` (lowercase) on a thread. Checks the cancel
/// flag for every file, so a superseded search stops almost immediately.
fn spawn_grep(root: PathBuf, q: String) -> GrepJob {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    thread::spawn(move || {
        let mut batch: Vec<GrepHit> = Vec::new();
        let mut sent = 0usize;
        let mut last = Instant::now();
        let walker = WalkBuilder::new(&root).hidden(false).git_ignore(true).filter_entry(keep).build();
        'outer: for e in walker.flatten() {
            if flag.load(Ordering::Relaxed) { return; }
            if !e.file_type().map_or(false, |t| t.is_file()) { continue; }
            if e.metadata().map_or(false, |m| m.len() > MAX_GREP_FILE) { continue; }
            let Ok(content) = fs::read_to_string(e.path()) else { continue };
            // most files don't contain the text at all: rule them out in one pass
            if !content.to_lowercase().contains(&q) { continue; }
            for (i, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&q) {
                    batch.push(GrepHit {
                        path: e.path().to_path_buf(),
                        line: i + 1,
                        text: line.trim().to_string(),
                    });
                    sent += 1;
                    if sent >= MAX_GREP { break 'outer; }
                }
            }
            if batch.len() >= 50 || last.elapsed() >= Duration::from_millis(40) {
                if tx.send(std::mem::take(&mut batch)).is_err() { return; }
                last = Instant::now();
            }
        }
        let _ = tx.send(batch);
    });
    GrepJob { rx, cancel }
}

// ============================================================
// Ranking
// ============================================================

fn is_boundary(hay: &str, pos: usize) -> bool {
    match hay[..pos].chars().last() {
        None => true,
        Some(c) => matches!(c, '/' | '.' | '_' | '-' | ' '),
    }
}

/// Score `needle` inside `hay` (both lowercase). `None` if it doesn't match.
/// A contiguous match beats a scattered one; prefixes and word starts win.
pub fn match_score(hay: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() { return Some(0); }
    if let Some(pos) = hay.find(needle) {
        let mut s = 600 - (pos as i32).min(200);
        if pos == 0 { s += 300; } else if is_boundary(hay, pos) { s += 150; }
        if hay.len() == needle.len() { s += 400; }
        return Some(s);
    }
    // scattered match: every letter in order
    let mut score = 0i32;
    let mut it = hay.char_indices();
    let mut prev_end: Option<usize> = None;
    for nc in needle.chars() {
        let mut found = false;
        for (i, hc) in it.by_ref() {
            if hc == nc {
                found = true;
                score += 10;
                if prev_end == Some(i) { score += 15; }
                if is_boundary(hay, i) { score += 10; }
                prev_end = Some(i + hc.len_utf8());
                break;
            }
            score -= 1;
        }
        if !found { return None; }
    }
    Some(score.clamp(1, 300))
}

/// Score one file against all tokens (every token must match). A hit in the
/// file *name* counts far more than a hit somewhere in the folder path.
fn score_entry(e: &Entry, tokens: &[String]) -> Option<i32> {
    let name = &e.rel[e.name_at..];
    let mut total = 0i32;
    for t in tokens {
        total += match match_score(name, t) {
            Some(s) => s + 1000,
            None => match_score(&e.rel, t)?,
        };
    }
    let depth = e.rel.matches('/').count() as i32;
    Some(total - depth * 3 - (e.rel.len() as i32) / 4)
}

/// Best `limit` files for `tokens`, best first. An empty query lists shallow,
/// short paths first.
fn rank(entries: &[Entry], tokens: &[String], limit: usize) -> Vec<PathBuf> {
    let mut scored: Vec<(i32, usize, usize)> = entries.iter().enumerate()
        .filter_map(|(i, e)| score_entry(e, tokens).map(|s| (s, e.rel.len(), i)))
        .collect();
    let by = |a: &(i32, usize, usize), b: &(i32, usize, usize)| {
        (Reverse(a.0), a.1, a.2).cmp(&(Reverse(b.0), b.1, b.2))
    };
    if scored.len() > limit {
        scored.select_nth_unstable_by(limit, by);
        scored.truncate(limit);
    }
    scored.sort_by(by);
    scored.into_iter().map(|(_, _, i)| entries[i].path.clone()).collect()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[&str]) -> Vec<Entry> {
        paths.iter().map(|p| make_entry(Path::new("/r"), PathBuf::from(format!("/r/{}", p)))).collect()
    }

    fn top(paths: &[&str], q: &str) -> Vec<String> {
        let toks: Vec<String> = q.to_lowercase().split_whitespace().map(String::from).collect();
        rank(&entries(paths), &toks, 50).into_iter()
            .map(|p| p.strip_prefix("/r").unwrap().to_string_lossy().trim_start_matches('/').to_string())
            .collect()
    }

    #[test]
    fn exact_file_name_beats_deep_path_matches() {
        let r = top(&["docs/maintainers/notes.txt", "src/deep/er/main_helper.rs", "src/main.rs", "main.rs"], "main.rs");
        assert_eq!(r[0], "main.rs");
        assert_eq!(r[1], "src/main.rs");
    }

    #[test]
    fn name_match_beats_folder_match() {
        let r = top(&["config/app.toml", "src/config.rs", "other/x.rs"], "config");
        assert_eq!(r[0], "src/config.rs");
        assert_eq!(r[1], "config/app.toml");
    }

    #[test]
    fn contiguous_beats_scattered() {
        let r = top(&["a_b_c_d.txt", "abcd.txt", "xabcdx.txt"], "abcd");
        assert_eq!(r[0], "abcd.txt");
        assert!(r.contains(&"a_b_c_d.txt".to_string()));
    }

    #[test]
    fn scattered_letters_still_match() {
        let r = top(&["src/finder.rs", "src/tree.rs"], "fdr");
        assert_eq!(r, ["src/finder.rs"]);
    }

    #[test]
    fn every_token_must_match() {
        let r = top(&["src/app.rs", "src/ui.rs", "tests/app.rs"], "src app");
        assert_eq!(r[0], "src/app.rs");
        assert!(!r.contains(&"src/ui.rs".to_string()));
    }

    #[test]
    fn root_prefix_is_not_searched() {
        assert!(top(&["src/main.rs"], "zzz").is_empty());
        let e = make_entry(Path::new("/home/user"), PathBuf::from("/home/user/src/a.rs"));
        assert_eq!(e.rel, "src/a.rs");
    }

    #[test]
    fn empty_query_lists_shallow_paths_first() {
        let r = top(&["a/b/c/d.txt", "z.txt", "a/y.txt"], "");
        assert_eq!(r, ["z.txt", "a/y.txt", "a/b/c/d.txt"]);
    }

    #[test]
    fn case_and_unicode_are_fine() {
        assert_eq!(top(&["Docs/README.md"], "readme")[0], "Docs/README.md");
        assert_eq!(top(&["مستندات/ملف.txt"], "ملف")[0], "مستندات/ملف.txt");
    }

    #[test]
    fn ranking_keeps_only_the_best_limit() {
        let paths: Vec<String> = (0..100).map(|i| format!("f{}/file{}.txt", i, i)).collect();
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        let toks = vec!["file".to_string()];
        assert_eq!(rank(&entries(&refs), &toks, 10).len(), 10);
    }

    #[test]
    fn scrolling_keeps_selection_visible() {
        let mut f = Finder::new();
        f.file_results = (0..100).map(|i| PathBuf::from(format!("/r/{}", i))).collect();
        f.clamp_scroll(10);
        assert_eq!(f.offset, 0);
        f.selected = 9; f.clamp_scroll(10);
        assert_eq!(f.offset, 0);
        f.selected = 10; f.clamp_scroll(10);
        assert_eq!(f.offset, 1);
        f.selected = 99; f.clamp_scroll(10);
        assert_eq!(f.offset, 90);
        f.selected = 5; f.clamp_scroll(10);
        assert_eq!(f.offset, 5);
        f.page_down(); assert_eq!(f.selected, 15);
        f.page_up(); f.page_up(); assert_eq!(f.selected, 0);
        f.last(); assert_eq!(f.selected, 99);
        f.move_by(50); assert_eq!(f.selected, 99);
    }

    fn grep_dir(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("anvil-grep-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (f, body) in files {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        dir
    }

    fn run_grep(f: &mut Finder, root: &Path) {
        f.update_grep(root);
        let t0 = Instant::now();
        while f.searching() && t0.elapsed() < Duration::from_secs(10) {
            f.poll();
            thread::sleep(Duration::from_millis(5));
        }
        f.poll();
    }

    #[test]
    fn live_grep_streams_hits_in_the_background() {
        let dir = grep_dir("hits", &[
            ("a.txt", "one\nNeedle here\nthree\n"),
            ("sub/b.txt", "nothing\n"),
            ("sub/c.rs", "let x = 1; // needle\n"),
            ("node_modules/d.js", "needle\n"),
            (".git/e", "needle\n"),
        ]);
        let mut f = Finder::new();
        f.open_grep(&dir);
        f.query = "NEEDLE".into();
        run_grep(&mut f, &dir);
        let mut got: Vec<(String, usize)> = f.grep_results.iter()
            .map(|h| (h.path.strip_prefix(&dir).unwrap().to_string_lossy().to_string(), h.line)).collect();
        got.sort();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(got, [("a.txt".to_string(), 2), ("sub/c.rs".to_string(), 1)]);
    }

    #[test]
    fn a_new_query_replaces_and_cancels_the_old_search() {
        let dir = grep_dir("cancel", &[("a.txt", "alpha\n"), ("b.txt", "beta\n")]);
        let mut f = Finder::new();
        f.open_grep(&dir);
        f.query = "alpha".into();
        f.update_grep(&dir);          // starts a search ...
        f.query = "beta".into();
        run_grep(&mut f, &dir);       // ... which the next one replaces
        let hits: Vec<String> = f.grep_results.iter().map(|h| h.text.clone()).collect();
        // empty query clears everything and starts nothing
        f.query.clear();
        f.update_grep(&dir);
        let cleared = f.grep_results.is_empty() && !f.searching();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(hits, ["beta"]);
        assert!(cleared);
    }

    #[test]
    fn live_grep_stops_at_the_hit_limit() {
        let body = "match\n".repeat(MAX_GREP + 200);
        let dir = grep_dir("limit", &[("big.txt", body.as_str())]);
        let mut f = Finder::new();
        f.open_grep(&dir);
        f.query = "match".into();
        run_grep(&mut f, &dir);
        let n = f.grep_results.len();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(n, MAX_GREP);
    }

    #[test]
    fn background_index_skips_junk_folders() {
        let dir = std::env::temp_dir().join(format!("anvil-finder-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for d in ["src", "node_modules/pkg", ".git/objects", "keep/target", "rust/target", ".cache/x"] {
            fs::create_dir_all(dir.join(d)).unwrap();
        }
        fs::write(dir.join("rust/Cargo.toml"), "").unwrap();
        for f in ["src/a.rs", "node_modules/pkg/i.js", ".git/objects/o", "keep/target/t.txt",
                  "rust/target/b.o", ".cache/x/c", "top.txt"] {
            fs::write(dir.join(f), "x").unwrap();
        }
        let rx = spawn_index(dir.clone());
        let mut got: Vec<String> = Vec::new();
        while let Ok(batch) = rx.recv() {
            for p in batch { got.push(p.strip_prefix(&dir).unwrap().to_string_lossy().to_string()); }
        }
        got.sort();
        let _ = fs::remove_dir_all(&dir);
        // a folder that is merely *called* target is kept; Cargo's build dir is not
        assert_eq!(got, ["keep/target/t.txt", "rust/Cargo.toml", "src/a.rs", "top.txt"]);
    }
}
