use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileTree {
    pub root: PathBuf,
    pub entries: Vec<TreeEntry>,
    pub selected: usize,
    pub scroll: u16,
    pub filter: String,
    pub visible: bool,
    pub width: u16,
    pub expanded: HashSet<PathBuf>,
    pub focus: bool,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut t = Self {
            root: root.clone(),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            filter: String::new(),
            visible: true,
            width: 30,
            expanded: HashSet::new(),
            focus: false,
        };
        t.expanded.insert(root);
        t.refresh();
        t
    }

    /// يغيّر جذر الشجرة ويعيد بناء القائمة (يُستخدم عندما يفتح المستخدم ملفاً في مجلد آخر)
    pub fn set_root(&mut self, new_root: PathBuf) {
        if self.root == new_root { return; }
        self.root = new_root.clone();
        self.expanded.clear();
        self.expanded.insert(new_root);
        self.selected = 0;
        self.scroll = 0;
        self.filter.clear();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        let root = self.root.clone();
        self.walk(&root, 0);
        if self.entries.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn walk(&mut self, dir: &Path, depth: usize) {
        let mut items: Vec<(PathBuf, bool, String)> = Vec::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                let is_dir = p.is_dir();
                items.push((p, is_dir, name));
            }
        }
        items.sort_by(|a, b| match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.2.to_lowercase().cmp(&b.2.to_lowercase()),
        });

        let filter = self.filter.to_lowercase();
        for (p, is_dir, name) in items {
            let matches = filter.is_empty() || is_dir || name.to_lowercase().contains(&filter);
            if !matches { continue; }

            let expanded = self.expanded.contains(&p);
            self.entries.push(TreeEntry {
                path: p.clone(), name, is_dir, depth, expanded,
            });
            if is_dir && expanded {
                self.walk(&p, depth + 1);
            }
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(e) = self.entries.get(self.selected).cloned() {
            if e.is_dir {
                if self.expanded.contains(&e.path) {
                    self.expanded.remove(&e.path);
                } else {
                    self.expanded.insert(e.path.clone());
                }
                self.refresh();
            }
        }
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.entries.get(self.selected).map(|e| e.path.clone())
    }

    pub fn selected_is_dir(&self) -> bool {
        self.entries.get(self.selected).map(|e| e.is_dir).unwrap_or(false)
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 { self.selected -= 1; }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() { self.selected += 1; }
    }

    pub fn clamp_scroll(&mut self, view_height: usize) {
        if view_height == 0 { return; }
        let sel = self.selected;
        let scroll = self.scroll as usize;
        if sel < scroll {
            self.scroll = sel as u16;
        } else if sel >= scroll + view_height {
            self.scroll = (sel + 1 - view_height) as u16;
        }
    }

    pub fn breadcrumb(&self) -> String {
        if let Some(e) = self.entries.get(self.selected) {
            let mut parts: Vec<String> = Vec::new();
            if let Ok(rel) = e.path.strip_prefix(&self.root) {
                for c in rel.components() {
                    parts.push(c.as_os_str().to_string_lossy().to_string());
                }
            } else {
                parts.push(e.path.to_string_lossy().to_string());
            }
            format!(" {}", parts.join(" › "))
        } else {
            format!(" {}", self.root.to_string_lossy())
        }
    }

    pub fn create_file(&mut self, name: &str) -> std::io::Result<PathBuf> {
        let dir = if let Some(e) = self.entries.get(self.selected) {
            if e.is_dir { e.path.clone() }
            else { e.path.parent().map(|p| p.to_path_buf()).unwrap_or(self.root.clone()) }
        } else { self.root.clone() };
        let full = dir.join(name);
        fs::write(&full, "")?;
        self.refresh();
        Ok(full)
    }

    pub fn create_dir(&mut self, name: &str) -> std::io::Result<()> {
        let dir = if let Some(e) = self.entries.get(self.selected) {
            if e.is_dir { e.path.clone() }
            else { e.path.parent().map(|p| p.to_path_buf()).unwrap_or(self.root.clone()) }
        } else { self.root.clone() };
        fs::create_dir_all(dir.join(name))?;
        self.refresh();
        Ok(())
    }

    pub fn delete_selected(&mut self) -> std::io::Result<()> {
        if let Some(e) = self.entries.get(self.selected).cloned() {
            if e.is_dir { fs::remove_dir_all(&e.path)?; }
            else { fs::remove_file(&e.path)?; }
            self.refresh();
        }
        Ok(())
    }
}

pub fn icon_for(name: &str, is_dir: bool, expanded: bool) -> &'static str {
    if is_dir {
        return if expanded { "\u{f07c}" } else { "\u{f07b}" };
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let lower = name.to_lowercase();
    match ext.as_str() {
        "rs" => "\u{e7a8}",
        "c" | "h" => "\u{e61e}",
        "cpp" | "hpp" | "cc" | "cxx" => "\u{e61d}",
        "py" | "pyi" => "\u{e73c}",
        "js" | "mjs" | "cjs" => "\u{e74e}",
        "jsx" => "\u{e7ba}",
        "ts" | "tsx" => "\u{e628}",
        "go" => "\u{e627}",
        "java" => "\u{e738}",
        "kt" => "\u{e634}",
        "rb" => "\u{e739}",
        "php" => "\u{e73d}",
        "swift" => "\u{e755}",
        "lua" => "\u{e620}",
        "sh" | "bash" | "zsh" | "fish" => "\u{f489}",
        "md" | "markdown" => "\u{e73e}",
        "json" => "\u{e60b}",
        "toml" => "\u{e615}",
        "yaml" | "yml" => "\u{e615}",
        "xml" => "\u{e619}",
        "html" | "htm" => "\u{e736}",
        "css" => "\u{e749}",
        "scss" | "sass" => "\u{e603}",
        "sql" => "\u{e706}",
        "lock" => "\u{f023}",
        "txt" => "\u{f15c}",
        "log" => "\u{f18d}",
        "pdf" => "\u{f1c1}",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => "\u{f1c5}",
        "mp4" | "mkv" | "avi" => "\u{f1c8}",
        "mp3" | "wav" => "\u{f1c7}",
        "zip" | "tar" | "gz" | "xz" => "\u{f410}",
        _ => {
            if lower == "makefile" { "\u{e673}" }
            else if lower == "dockerfile" { "\u{e7b0}" }
            else if lower == "license" { "\u{e60a}" }
            else if lower.starts_with(".git") { "\u{e702}" }
            else { "\u{f15b}" }
        }
    }
}
