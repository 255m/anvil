use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

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

pub struct Finder {
    pub kind: FinderKind,
    pub query: String,
    pub visible: bool,
    pub selected: usize,
    pub all_files: Vec<PathBuf>,
    pub file_results: Vec<PathBuf>,
    pub grep_results: Vec<GrepHit>,
}

impl Finder {
    pub fn new() -> Self {
        Self {
            kind: FinderKind::File,
            query: String::new(),
            visible: false,
            selected: 0,
            all_files: Vec::new(),
            file_results: Vec::new(),
            grep_results: Vec::new(),
        }
    }

    pub fn open_files(&mut self, root: &Path) {
        self.kind = FinderKind::File;
        self.query.clear();
        self.selected = 0;
        self.visible = true;
        self.all_files.clear();
        self.file_results.clear();
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .build();
        for e in walker.flatten() {
            if e.file_type().map_or(false, |t| t.is_file()) {
                self.all_files.push(e.path().to_path_buf());
            }
        }
        self.update_files();
    }

    pub fn open_grep(&mut self, _root: &Path) {
        self.kind = FinderKind::Grep;
        self.query.clear();
        self.selected = 0;
        self.visible = true;
        self.grep_results.clear();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
    }

    pub fn update_files(&mut self) {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            self.file_results = self.all_files.iter().take(300).cloned().collect();
            return;
        }
        let mut scored: Vec<(i32, PathBuf)> = self
            .all_files
            .iter()
            .filter_map(|p| {
                let s = p.to_string_lossy().to_lowercase();
                fuzzy(&s, &q).map(|sc| (sc, p.clone()))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.file_results = scored.into_iter().take(300).map(|(_, p)| p).collect();
        if self.selected >= self.file_results.len() {
            self.selected = 0;
        }
    }

    pub fn update_grep(&mut self, root: &Path) {
        self.grep_results.clear();
        self.selected = 0;
        if self.query.is_empty() {
            return;
        }
        let q = self.query.clone();
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .build();
        'outer: for e in walker.flatten() {
            if !e.file_type().map_or(false, |t| t.is_file()) {
                continue;
            }
            let path = e.path().to_path_buf();
            if let Ok(content) = fs::read_to_string(&path) {
                if content.len() > 2_000_000 {
                    continue;
                }
                for (i, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&q.to_lowercase()) {
                        self.grep_results.push(GrepHit {
                            path: path.clone(),
                            line: i + 1,
                            text: line.trim().to_string(),
                        });
                        if self.grep_results.len() >= 500 {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let len = match self.kind {
            FinderKind::File => self.file_results.len(),
            FinderKind::Grep => self.grep_results.len(),
        };
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }
}

/// Simple subsequence fuzzy matcher. Returns a score (higher = better), or None.
fn fuzzy(haystack: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut score = 0i32;
    let mut hi = haystack.chars();
    for nc in needle.chars() {
        let mut found = false;
        for hc in hi.by_ref() {
            if hc == nc {
                found = true;
                score += 1;
                break;
            }
        }
        if !found {
            return None;
        }
    }
    Some(score)
}
