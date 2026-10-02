use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Instant, SystemTime};

use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::buffer::Buffer;
use crate::config::{Config, Theme, THEME_NAMES};
use crate::finder::{Finder, FinderKind};
use crate::syntax::{self, Language};
use crate::terminal::{TermKind, Terminal};
use crate::tree::FileTree;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode { Normal, Insert, Visual, VisualLine, Command }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputKind {
    None, NewFile, GotoLine, SaveAs, TreeNewFile, TreeNewFolder,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane { Tree, SideTerm, BottomTerm, Editor }

pub struct App {
    pub buffers: Vec<Buffer>,
    pub current: usize,
    pub mode: Mode,
    pub command: String,
    pub command_prefix: char,
    pub message: String,
    pub config: Config,
    pub theme: Theme,
    pub tree: FileTree,
    pub tree_history: Vec<PathBuf>,
    pub finder: Finder,
    pub leader: bool,
    pub pending_count: String,
    pub pending_op: Option<char>,
    pub pending_g: bool,
    pub pending_f: Option<char>,
    pub should_quit: bool,
    pub start_time: Instant,
    pub search: Option<String>,
    pub search_index: usize,
    pub search_hits: Vec<(usize, usize)>,
    pub clipboard: String,
    pub registers: HashMap<char, String>,
    pub show_help: bool,
    pub last_find: Option<(char, bool)>,
    pub area_height: u16,
    pub last_tree_area: Rect,
    pub last_editor_area: Rect,
    pub last_text_area: Rect,
    pub last_tabs_area: Rect,
    pub last_side_term_area: Rect,
    pub last_bottom_term_area: Rect,
    pub last_finder_list_area: Rect,
    pub last_theme_list_area: Rect,
    pub last_leader_area: Rect,
    pub editor_view_h: u16,
    pub cursor_screen_pos: (u16, u16),
    pub mouse_dragging: bool,
    pub last_click: Option<(u16, u16, Instant)>,
    pub theme_picker_visible: bool,
    pub theme_picker_index: usize,
    pub tree_focus: bool,
    pub input_kind: InputKind,
    pub input_text: String,
    pub confirm_delete: Option<PathBuf>,
    pub completion_active: bool,
    pub completion_items: Vec<String>,
    pub completion_index: usize,
    pub completion_prefix: String,
    pub dashboard_dismissed: bool,
    pub clipboard_request: Option<String>,

    pub side_term: Option<Terminal>,
    pub bottom_term: Option<Terminal>,
    pub active_pane: Pane,

    pub music_title: String,
    pub music_rx: Receiver<String>,

    pub config_mtime: Option<SystemTime>,
    pub last_config_check: Instant,

    pub diagnostics: Vec<(usize, String)>,
    pub last_diag_check: Instant,
}

fn is_scratch(b: &Buffer) -> bool {
    b.path.is_none() && !b.modified && b.lines.len() == 1 && b.lines[0].is_empty()
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        return PathBuf::from(home).join(rest);
    }
    if path == "~" {
        let home = std::env::var("HOME").unwrap_or_default();
        return PathBuf::from(home);
    }
    PathBuf::from(path)
}

fn spawn_music_thread() -> Receiver<String> {
    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        loop {
            let title = std::process::Command::new("playerctl")
                .args(&["metadata", "--format", "{{title}}"])
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            if tx.send(title).is_err() { break; }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    });
    rx
}

impl App {
    pub fn new(file: Option<String>) -> Self {
        let config = Config::load();
        let mut theme = Theme::by_name(&config.theme);
        config.apply_color_overrides(&mut theme);

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut tree = FileTree::new(cwd);
        tree.visible = false;
        tree.width = config.tree_width;

        let has_file = file.is_some();
        let mut startup_dir: Option<PathBuf> = None;

        let buffers = if let Some(f) = file {
            let p = PathBuf::from(&f);
            tree.visible = config.tree;
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    startup_dir = Some(parent.to_path_buf());
                }
            }
            if p.exists() {
                match Buffer::from_file(p.clone()) {
                    Ok(b) => vec![b],
                    Err(_) => {
                        let mut b = Buffer::empty();
                        b.path = Some(p);
                        vec![b]
                    }
                }
            } else {
                let mut b = Buffer::empty();
                b.path = Some(p);
                vec![b]
            }
        } else {
            vec![Buffer::empty()]
        };

        if let Some(dir) = startup_dir {
            let abs = if dir.is_absolute() { dir }
                else {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(dir)
                };
            tree.set_root(abs);
        }

        let theme_picker_index = THEME_NAMES
            .iter().position(|n| *n == config.theme).unwrap_or(0);

        let music_rx = if config.music_enabled { spawn_music_thread() } else { channel().1 };

        let initial_mtime = std::fs::metadata(&config.path)
            .ok()
            .and_then(|m| m.modified().ok());

        let app = Self {
            buffers, current: 0, mode: Mode::Normal,
            command: String::new(), command_prefix: ':',
            message: String::new(), config, theme, tree,
            tree_history: Vec::new(),
            finder: Finder::new(),
            leader: false, pending_count: String::new(),
            pending_op: None, pending_g: false, pending_f: None,
            should_quit: false, start_time: Instant::now(),
            search: None, search_index: 0, search_hits: Vec::new(),
            clipboard: String::new(), registers: HashMap::new(),
            show_help: false, last_find: None, area_height: 24,
            last_tree_area: Rect::default(),
            last_editor_area: Rect::default(),
            last_text_area: Rect::default(),
            last_tabs_area: Rect::default(),
            last_side_term_area: Rect::default(),
            last_bottom_term_area: Rect::default(),
            last_finder_list_area: Rect::default(),
            last_theme_list_area: Rect::default(),
            last_leader_area: Rect::default(),
            editor_view_h: 20,
            cursor_screen_pos: (0, 0),
            mouse_dragging: false, last_click: None,
            theme_picker_visible: false, theme_picker_index,
            tree_focus: false,
            input_kind: InputKind::None,
            input_text: String::new(),
            confirm_delete: None,
            completion_active: false,
            completion_items: Vec::new(),
            completion_index: 0,
            completion_prefix: String::new(),
            dashboard_dismissed: has_file,
            clipboard_request: None,
            side_term: None,
            bottom_term: None,
            active_pane: Pane::Editor,
            music_title: String::new(),
            music_rx,
            config_mtime: initial_mtime,
            last_config_check: Instant::now(),
            diagnostics: Vec::new(),
            last_diag_check: Instant::now(),
        };
        app.config.save_default_if_missing();
        app
    }

    pub fn buf(&mut self) -> &mut Buffer { &mut self.buffers[self.current] }
    pub fn buf_ref(&self) -> &Buffer { &self.buffers[self.current] }

    pub fn is_dashboard(&self) -> bool {
        if self.dashboard_dismissed { return false; }
        if self.buffers.len() != 1 || self.current != 0 { return false; }
        is_scratch(self.buf_ref())
    }

    pub fn take_clipboard_request(&mut self) -> Option<String> {
        self.clipboard_request.take()
    }

    fn system_copy(&mut self, text: &str) {
        self.clipboard = text.to_string();
        self.clipboard_request = Some(text.to_string());
    }

    pub fn desired_tree_width(&self) -> u16 {
        let mut max_w = 20usize;
        for e in &self.tree.entries {
            let w = e.depth * 2 + 4 + e.name.chars().count();
            if w > max_w { max_w = w; }
        }
        (max_w + 2).min(self.config.tree_width as usize) as u16
    }

    pub fn pump_music(&mut self) {
        while let Ok(t) = self.music_rx.try_recv() {
            self.music_title = t;
        }
    }

    pub fn save_config_now(&mut self) {
        if self.config.save().is_ok() {
            if let Ok(meta) = std::fs::metadata(&self.config.path) {
                if let Ok(mtime) = meta.modified() {
                    self.config_mtime = Some(mtime);
                }
            }
        }
    }

    pub fn pump_config(&mut self) {
        if self.last_config_check.elapsed().as_millis() < 500 { return; }
        self.last_config_check = Instant::now();

        let Ok(meta) = std::fs::metadata(&self.config.path) else { return; };
        let Ok(mtime) = meta.modified() else { return; };

        if self.config_mtime == Some(mtime) { return; }

        let is_first = self.config_mtime.is_none();
        self.config_mtime = Some(mtime);
        if is_first { return; }

        self.config.reload();
        self.theme = Theme::by_name(&self.config.theme);
        self.config.apply_color_overrides(&mut self.theme);
        self.tree.width = self.config.tree_width;

        self.message = "config auto-reloaded".into();
    }

    pub fn pump_diagnostics(&mut self) {
        if self.last_diag_check.elapsed().as_millis() < 400 { return; }
        self.last_diag_check = Instant::now();

        let filename = self.buf_ref().filename();
        let lang = syntax::detect(&filename);
        if matches!(lang, Language::None) {
            if !self.diagnostics.is_empty() {
                self.diagnostics.clear();
            }
            return;
        }

        let lines: Vec<String> = self.buf_ref().lines.clone();
        let known = syntax::build_known_words(&lines);

        let mut diags: Vec<(usize, String)> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let issues = syntax::diagnose_line(line, lang, &known);
            for issue in issues {
                diags.push((i, issue));
            }
        }
        self.diagnostics = diags;
    }

    // ==================== TREE NAVIGATION ====================
    fn tree_push_history(&mut self) {
        self.tree_history.push(self.tree.root.clone());
        if self.tree_history.len() > 64 {
            self.tree_history.remove(0);
        }
    }

    pub fn tree_set_root(&mut self, path: PathBuf) {
        if self.tree.root == path { return; }
        self.tree_push_history();
        self.tree.set_root(path);
    }

    fn tree_go_back(&mut self) {
        if let Some(prev) = self.tree_history.pop() {
            self.tree.set_root(prev);
            self.sync_tree_scroll();
            self.message = "tree ← back".into();
        } else {
            self.message = "tree: no history".into();
        }
    }

    fn tree_go_up(&mut self) {
        if let Some(parent) = self.tree.root.parent() {
            let p = parent.to_path_buf();
            self.tree_push_history();
            self.tree.set_root(p);
            self.sync_tree_scroll();
            self.message = "tree ↑ parent".into();
        } else {
            self.message = "already at root".into();
        }
    }

    fn tree_go_home(&mut self) {
        if let Ok(home) = std::env::var("HOME") {
            self.tree_set_root(PathBuf::from(home));
            self.sync_tree_scroll();
            self.message = "tree → home".into();
        }
    }

    // ==================== RUN FILE ====================
    pub fn run_file(&mut self) {
        let Some(path) = self.buf_ref().path.clone() else {
            self.message = "save the file first".into();
            return;
        };
        let _ = self.buf().save();

        let lang = syntax::detect(&path.to_string_lossy());

        let cmd = match lang {
            Language::Python => format!("python3 '{}'", path.display()),
            Language::JavaScript => format!("node '{}'", path.display()),
            Language::TypeScript => format!("npx ts-node '{}'", path.display()),
            Language::Go => format!("go run '{}'", path.display()),
            Language::Shell => format!("bash '{}'", path.display()),
            Language::Ruby => format!("ruby '{}'", path.display()),
            Language::Php => format!("php '{}'", path.display()),
            Language::Lua => format!("lua '{}'", path.display()),
            Language::C => {
                let out = "/tmp/anvil_c_out";
                format!("gcc '{}' -o {} && {}", path.display(), out, out)
            }
            Language::Cpp => {
                let out = "/tmp/anvil_cpp_out";
                format!("g++ '{}' -o {} && {}", path.display(), out, out)
            }
            Language::Rust => {
                let out = "/tmp/anvil_rs_out";
                format!("rustc '{}' -o {} && {}", path.display(), out, out)
            }
            Language::Java => {
                let dir = path.parent().unwrap_or(&path);
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                format!(
                    "cd '{}' && javac '{}' && java {}",
                    dir.display(), path.display(), name
                )
            }
            Language::Nim => format!("nim c -r '{}'", path.display()),
            Language::Zig => format!("zig run '{}'", path.display()),
            _ => {
                self.message = format!("can't run {}", lang.name());
                return;
            }
        };

        let h = self.config.term_bottom_height;
        self.ensure_bottom_term(h, 100);
        if let Some(t) = &mut self.bottom_term {
            t.visible = true;
            t.send_str(&format!("{}\n", cmd));
        }
        self.active_pane = Pane::BottomTerm;
        self.message = format!("running {}", path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()));
    }

    fn install_buffer(&mut self, b: Buffer) {
        if let Some(p) = &b.path {
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    let abs = if parent.is_absolute() { parent.to_path_buf() }
                        else {
                            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                                .join(parent)
                        };
                    if !abs.starts_with(&self.tree.root) {
                        self.tree_push_history();
                        self.tree.set_root(abs);
                    }
                }
            }
        }
        if let Some(idx) = self.buffers.iter().position(is_scratch) {
            self.buffers[idx] = b;
            self.current = idx;
        } else {
            self.buffers.push(b);
            self.current = self.buffers.len() - 1;
        }
        self.dashboard_dismissed = true;
    }

    fn ensure_side_term(&mut self, rows: u16, cols: u16) {
        if self.side_term.is_none() {
            match Terminal::spawn(TermKind::Left, None, rows, cols) {
                Ok(t) => self.side_term = Some(t),
                Err(e) => self.message = format!("term error: {}", e),
            }
        }
    }

    fn ensure_bottom_term(&mut self, rows: u16, cols: u16) {
        if self.bottom_term.is_none() {
            match Terminal::spawn(TermKind::Bottom, None, rows, cols) {
                Ok(t) => self.bottom_term = Some(t),
                Err(e) => self.message = format!("term error: {}", e),
            }
        }
    }

    pub fn pump_terminals(&mut self) {
        if let Some(t) = &mut self.side_term { t.pump(); }
        if let Some(t) = &mut self.bottom_term { t.pump(); }
        if self.side_term.as_ref().map(|t| t.exited).unwrap_or(false) {
            self.side_term = None;
            if self.active_pane == Pane::SideTerm { self.active_pane = Pane::Editor; }
        }
        if self.bottom_term.as_ref().map(|t| t.exited).unwrap_or(false) {
            self.bottom_term = None;
            if self.active_pane == Pane::BottomTerm { self.active_pane = Pane::Editor; }
        }
    }

    fn has_visible_terminal(&self) -> bool {
        self.side_term.as_ref().map(|t| t.visible).unwrap_or(false)
            || self.bottom_term.as_ref().map(|t| t.visible).unwrap_or(false)
    }

    pub fn has_visible_terminal_pub(&self) -> bool { self.has_visible_terminal() }

    // ================= PASTE =================
    pub fn handle_paste(&mut self, text: String) {
        if text.is_empty() { return; }

        if self.active_pane == Pane::SideTerm || self.active_pane == Pane::BottomTerm {
            let t = match self.active_pane {
                Pane::SideTerm => self.side_term.as_mut(),
                Pane::BottomTerm => self.bottom_term.as_mut(),
                _ => None,
            };
            if let Some(t) = t {
                t.send_str("\x1b[200~");
                t.send_str(&text);
                t.send_str("\x1b[201~");
            }
            return;
        }

        if self.input_kind != InputKind::None {
            let cleaned: String = text.chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
            self.input_text.push_str(&cleaned);
            return;
        }

        if self.finder.visible {
            let cleaned: String = text.chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
            self.finder.query.push_str(&cleaned);
            self.refresh_finder();
            return;
        }

        if matches!(self.mode, Mode::Command) {
            let cleaned: String = text.chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
            self.command.push_str(&cleaned);
            return;
        }

        if self.mode == Mode::Normal { self.mode = Mode::Insert; }
        self.message.clear();
        self.insert_pasted_text(&text);
    }

    fn insert_pasted_text(&mut self, text: &str) {
        self.completion_active = false;
        let clean = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = clean.split('\n').collect();

        if lines.len() == 1 {
            if !lines[0].is_empty() { self.buf().insert_str(lines[0]); }
        } else {
            for (i, raw) in lines.iter().enumerate() {
                if i == 0 {
                    if !raw.is_empty() { self.buf().insert_str(raw); }
                } else {
                    self.buf().newline();
                    if !raw.is_empty() { self.buf().insert_str(raw); }
                }
            }
        }
        self.buf().clamp_cursor();
        self.ensure_visible();
        self.dashboard_dismissed = true;
    }

    // ================= KEY =================
    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.confirm_delete.is_some() { self.handle_confirm_key(key); return; }
        if self.input_kind != InputKind::None { self.handle_input_key(key); return; }
        if self.theme_picker_visible { self.handle_theme_picker_key(key); return; }
        if self.finder.visible { self.handle_finder_key(key); return; }
        if self.show_help { self.show_help = false; return; }
        if self.leader { self.handle_leader_key(key); return; }

        if self.try_custom_keybind(&key) { return; }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('w') = key.code {
                self.pending_count = "w".into();
                return;
            }
            if self.pending_count == "w" {
                self.pending_count.clear();
                match key.code {
                    KeyCode::Char('h') | KeyCode::Left => self.move_pane_left(),
                    KeyCode::Char('l') | KeyCode::Right => self.move_pane_right(),
                    KeyCode::Char('j') | KeyCode::Down => self.move_pane_down(),
                    KeyCode::Char('k') | KeyCode::Up => self.move_pane_up(),
                    _ => {}
                }
                return;
            }
        }

        if self.active_pane == Pane::SideTerm || self.active_pane == Pane::BottomTerm {
            if let KeyCode::Esc = key.code { self.active_pane = Pane::Editor; return; }
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if let KeyCode::Char('v') = key.code {
                    let text = self.clipboard.clone();
                    if !text.is_empty() {
                        if let Some(t) = match self.active_pane {
                            Pane::SideTerm => self.side_term.as_mut(),
                            Pane::BottomTerm => self.bottom_term.as_mut(),
                            _ => None,
                        } {
                            t.send_str("\x1b[200~");
                            t.send_str(&text);
                            t.send_str("\x1b[201~");
                        }
                    }
                    return;
                }
            }
            if let Some(b) = key_event_to_bytes(&key) {
                if let Some(t) = match self.active_pane {
                    Pane::SideTerm => self.side_term.as_mut(),
                    Pane::BottomTerm => self.bottom_term.as_mut(),
                    _ => None,
                } { t.send_bytes(&b); }
            }
            return;
        }

        if self.tree_focus && self.tree.visible {
            if let KeyCode::Esc = key.code {
                self.tree_focus = false;
                self.active_pane = Pane::Editor;
                return;
            }
            self.handle_tree_key(key);
            return;
        }

        if matches!(self.mode, Mode::Normal) {
            if let KeyCode::Char(' ') = key.code {
                self.leader = true;
                return;
            }
        }
        if self.is_dashboard() {
            if self.handle_dashboard_key(key) { return; }
            if let KeyCode::Esc = key.code { return; }
            self.dashboard_dismissed = true;
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Visual | Mode::VisualLine => self.handle_visual(key),
            Mode::Command => self.handle_command(key),
        }
    }

    fn try_custom_keybind(&mut self, key: &KeyEvent) -> bool {
        let action = match self.match_keybind(key) {
            Some(a) => a, None => return false,
        };
        self.execute_action(&action);
        true
    }

    fn match_keybind(&self, key: &KeyEvent) -> Option<String> {
        for kb in &self.config.keybinds {
            if keybind_matches(&kb.key, key) { return Some(kb.action.clone()); }
        }
        None
    }

    fn execute_action(&mut self, action: &str) {
        let action = action.trim();
        if action == "term" { self.open_bottom_term(None); }
        else if let Some(dir) = action.strip_prefix("term:") { self.open_bottom_term(Some(dir.trim())); }
        else if action == "sterm" { self.open_side_term(None); }
        else if let Some(dir) = action.strip_prefix("sterm:") { self.open_side_term(Some(dir.trim())); }
        else if let Some(cmd) = action.strip_prefix("run:") {
            self.open_bottom_term(None);
            if let Some(t) = &mut self.bottom_term { t.send_str(cmd.trim()); t.send_str("\n"); }
        } else if let Some(path) = action.strip_prefix("edit:") {
            let p = expand_tilde(path.trim());
            let abs = if p.is_absolute() { p }
                else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
            self.open_file(abs);
        } else if let Some(path) = action.strip_prefix("cd:") {
            let p = expand_tilde(path.trim());
            let abs = if p.is_absolute() { p }
                else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
            self.message = format!("cd: {}", abs.display());
            self.tree_set_root(abs);
            self.tree.visible = true;
            self.config.tree = true;
            self.tree_focus = true;
            self.active_pane = Pane::Tree;
        } else if action == "new" { self.open_input(InputKind::NewFile, ""); }
        else if action == "save" {
            match self.buf().save() {
                Ok(_) => self.message = "written".into(),
                Err(e) => self.message = format!("write error: {}", e),
            }
        } else { self.message = format!("unknown action: {}", action); }
    }

    fn open_bottom_term(&mut self, dir: Option<&str>) {
        let h = self.config.term_bottom_height;
        self.ensure_bottom_term(h, 100);
        if let Some(t) = &mut self.bottom_term { t.visible = true; }
        self.active_pane = Pane::BottomTerm;
        if let Some(d) = dir {
            if !d.is_empty() {
                let p = expand_tilde(d);
                if let Some(t) = &mut self.bottom_term {
                    t.send_str(&format!("cd {}\n", shell_escape(&p.to_string_lossy())));
                }
            }
        }
    }

    fn open_side_term(&mut self, dir: Option<&str>) {
        let w = self.config.term_side_width;
        let h = self.area_height.saturating_sub(4).max(8);
        self.ensure_side_term(h, w);
        if let Some(t) = &mut self.side_term { t.visible = true; }
        self.active_pane = Pane::SideTerm;
        if let Some(d) = dir {
            if !d.is_empty() {
                let p = expand_tilde(d);
                if let Some(t) = &mut self.side_term {
                    t.send_str(&format!("cd {}\n", shell_escape(&p.to_string_lossy())));
                }
            }
        }
    }

    fn move_pane_left(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Editor => if self.tree.visible { Pane::Tree } else { Pane::Editor },
            Pane::SideTerm => Pane::Editor,
            Pane::BottomTerm => Pane::Editor,
            other => other,
        };
        self.tree_focus = self.active_pane == Pane::Tree;
    }

    fn move_pane_right(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Tree | Pane::Editor => {
                if self.side_term.as_ref().map(|t| t.visible).unwrap_or(false) {
                    Pane::SideTerm
                } else {
                    Pane::Editor
                }
            }
            other => other,
        };
        self.tree_focus = false;
    }

    fn move_pane_down(&mut self) {
        if (self.active_pane == Pane::Editor || self.active_pane == Pane::Tree)
            && self.bottom_term.as_ref().map(|t| t.visible).unwrap_or(false)
        {
            self.active_pane = Pane::BottomTerm;
            self.tree_focus = false;
        }
    }

    fn move_pane_up(&mut self) {
        if self.active_pane == Pane::BottomTerm { self.active_pane = Pane::Editor; }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(p) = self.confirm_delete.take() {
                    let res = if p.is_dir() { std::fs::remove_dir_all(&p) }
                              else { std::fs::remove_file(&p) };
                    match res {
                        Ok(_) => { self.message = format!("deleted: {}", p.display()); self.tree.refresh(); }
                        Err(e) => self.message = format!("delete error: {}", e),
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm_delete = None;
                self.message = "cancelled".into();
            }
            _ => {}
        }
    }

    fn open_input(&mut self, kind: InputKind, initial: &str) {
        self.input_kind = kind;
        self.input_text = initial.to_string();
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_kind = InputKind::None;
                self.input_text.clear();
            }
            KeyCode::Enter => {
                let text = self.input_text.clone();
                let kind = self.input_kind;
                self.input_kind = InputKind::None;
                self.input_text.clear();

                match kind {
                    InputKind::NewFile => {
                        if text.trim().is_empty() { return; }
                        let p = PathBuf::from(text.trim());
                        let abs = if p.is_absolute() { p }
                            else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
                        if !abs.exists() {
                            if let Some(parent) = abs.parent() { let _ = std::fs::create_dir_all(parent); }
                            let _ = std::fs::write(&abs, "");
                        }
                        match Buffer::from_file(abs.clone()) {
                            Ok(b) => { self.install_buffer(b); self.message = format!("new: {}", abs.display()); }
                            Err(e) => self.message = format!("error: {}", e),
                        }
                    }
                    InputKind::TreeNewFile => {
                        if text.trim().is_empty() { return; }
                        match self.tree.create_file(text.trim()) {
                            Ok(p) => { self.message = format!("created: {}", p.display()); }
                            Err(e) => self.message = format!("error: {}", e),
                        }
                    }
                    InputKind::TreeNewFolder => {
                        if text.trim().is_empty() { return; }
                        match self.tree.create_dir(text.trim()) {
                            Ok(_) => { self.message = format!("created dir: {}", text.trim()); }
                            Err(e) => self.message = format!("error: {}", e),
                        }
                    }
                    InputKind::SaveAs => {
                        if text.trim().is_empty() { return; }
                        let p = PathBuf::from(text.trim());
                        let abs = if p.is_absolute() { p }
                            else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
                        match self.buf().save_as(abs.clone()) {
                            Ok(_) => { self.tree.refresh(); self.message = format!("written: {}", abs.display()); }
                            Err(e) => self.message = format!("error: {}", e),
                        }
                    }
                    InputKind::GotoLine => {
                        if let Ok(n) = text.trim().parse::<usize>() {
                            self.buf().goto_line(n);
                            self.ensure_visible();
                        }
                    }
                    InputKind::None => {}
                }
            }
            KeyCode::Backspace => { self.input_text.pop(); }
            KeyCode::Char(c) => { self.input_text.push(c); }
            _ => {}
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> bool {
        let KeyCode::Char(c) = key.code else { return false; };
        match c {
            'n' => { self.open_input(InputKind::NewFile, ""); true }
            'f' => { let r = self.tree.root.clone(); self.finder.open_files(&r); true }
            'F' => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                self.finder.open_files(&PathBuf::from(home)); true
            }
            'g' => { let r = self.tree.root.clone(); self.finder.open_grep(&r); true }
            'e' => { self.tree.visible = true; self.config.tree = true; self.tree_focus = true; self.active_pane = Pane::Tree; true }
            't' => { self.open_theme_picker(); true }
            'h' => { self.show_help = true; true }
            'q' => { self.should_quit = true; true }
            _ => false,
        }
    }

    fn open_theme_picker(&mut self) {
        self.theme_picker_visible = true;
        self.theme_picker_index = THEME_NAMES
            .iter().position(|n| *n == self.theme.name).unwrap_or(0);
    }

    fn handle_theme_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => { self.theme_picker_visible = false; }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.theme_picker_index > 0 { self.theme_picker_index -= 1; }
                else { self.theme_picker_index = THEME_NAMES.len() - 1; }
                self.preview_theme(THEME_NAMES[self.theme_picker_index]);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.theme_picker_index = (self.theme_picker_index + 1) % THEME_NAMES.len();
                self.preview_theme(THEME_NAMES[self.theme_picker_index]);
            }
            KeyCode::Enter => {
                let name = THEME_NAMES[self.theme_picker_index];
                self.preview_theme(name);
                self.theme_picker_visible = false;
                self.message = format!("theme: {}", name);
                self.save_config_now();
            }
            _ => {}
        }
    }

    fn preview_theme(&mut self, name: &str) {
        let mut t = Theme::by_name(name);
        self.config.apply_color_overrides(&mut t);
        self.theme = t;
        self.config.theme = name.to_string();
    }

    fn handle_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.tree.move_down(); self.sync_tree_scroll();
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.tree.move_up(); self.sync_tree_scroll();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.tree.selected_is_dir() { self.tree.toggle_selected(); self.sync_tree_scroll(); }
                else if let Some(p) = self.tree.selected_path() {
                    self.open_file(p);
                    self.tree_focus = false;
                    self.active_pane = Pane::Editor;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.tree.selected_is_dir() { self.tree.toggle_selected(); }
            }
            KeyCode::Enter => {
                if self.tree.selected_is_dir() { self.tree.toggle_selected(); self.sync_tree_scroll(); }
                else if let Some(p) = self.tree.selected_path() {
                    self.open_file(p);
                    self.tree_focus = false;
                    self.active_pane = Pane::Editor;
                }
            }
            KeyCode::Char('q') => {
                self.tree.visible = false;
                self.config.tree = false;
                self.tree_focus = false;
                self.active_pane = Pane::Editor;
            }
            KeyCode::Char('a') => { self.open_input(InputKind::TreeNewFile, ""); }
            KeyCode::Char('f') => { self.open_input(InputKind::TreeNewFolder, ""); }
            KeyCode::Char('d') => {
                if let Some(p) = self.tree.selected_path() {
                    self.confirm_delete = Some(p);
                }
            }
            KeyCode::Backspace => { self.tree_go_back(); }
            KeyCode::Char('-') => { self.tree_go_up(); }
            KeyCode::Char('H') | KeyCode::Char('~') => { self.tree_go_home(); }
            KeyCode::Char('R') => {
                self.tree.refresh();
                self.message = "tree: refreshed".into();
            }
            _ => {}
        }
    }

    fn sync_tree_scroll(&mut self) {
        let h = (self.last_tree_area.height as usize).saturating_sub(3);
        self.tree.clamp_scroll(h);
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if ctrl {
            match key.code {
                KeyCode::Char('a') => {
                    let last = self.buf_ref().line_count().saturating_sub(1);
                    let last_len = self.buf_ref().line_chars(last);
                    self.buf().visual_anchor = Some((0, 0));
                    self.buf().cursor = (last, last_len);
                    self.mode = Mode::Visual;
                    self.message = "select all".into();
                    return;
                }
                KeyCode::Char('c') => {
                    let text = if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                        self.buf().yank_selection()
                    } else {
                        self.buf_ref().current_line().to_string()
                    };
                    self.system_copy(&text);
                    self.message = "copied".into();
                    if matches!(self.mode, Mode::Visual | Mode::VisualLine) { self.mode = Mode::Normal; }
                    return;
                }
                KeyCode::Char('x') => {
                    let text = if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                        self.buf().delete_selection()
                    } else {
                        self.buf().delete_line()
                    };
                    self.system_copy(&text);
                    self.mode = Mode::Normal;
                    self.message = "cut".into();
                    return;
                }
                KeyCode::Char('v') => {
                    let text = self.clipboard.clone();
                    if !text.is_empty() { self.buf().paste(&text, true); }
                    return;
                }
                KeyCode::Char('g') => { self.open_input(InputKind::GotoLine, ""); return; }
                KeyCode::Char('s') => { self.open_input(InputKind::SaveAs, ""); return; }
                KeyCode::Char('d') => {
                    let n = (self.editor_view_h as usize) / 2;
                    for _ in 0..n { self.buf().move_down(); }
                    self.ensure_visible(); return;
                }
                KeyCode::Char('u') => {
                    let n = (self.editor_view_h as usize) / 2;
                    for _ in 0..n { self.buf().move_up(); }
                    self.ensure_visible(); return;
                }
                KeyCode::Char('f') => {
                    let n = self.editor_view_h as usize;
                    for _ in 0..n { self.buf().move_down(); }
                    self.ensure_visible(); return;
                }
                KeyCode::Char('b') => {
                    let n = self.editor_view_h as usize;
                    for _ in 0..n { self.buf().move_up(); }
                    self.ensure_visible(); return;
                }
                KeyCode::Char('r') => { self.buf().redo(); return; }
                _ => {}
            }
        }

        if let KeyCode::Char(c) = key.code {
            if !ctrl && c.is_ascii_digit() && (c != '0' || !self.pending_count.is_empty()) {
                self.pending_count.push(c);
                return;
            }
        }

        if let Some(op) = self.pending_op {
            self.pending_op = None;
            if let KeyCode::Char(c) = key.code {
                self.apply_operator(op, c);
                self.pending_count.clear();
                return;
            }
        }
        if self.pending_g {
            self.pending_g = false;
            if let KeyCode::Char('g') = key.code {
                let count = self.take_count();
                if count > 0 { self.buf().goto_line(count); } else { self.buf().move_file_start(); }
                self.ensure_visible();
                return;
            }
        }
        if let Some(prefix) = self.pending_f.take() {
            if let KeyCode::Char(c) = key.code {
                match prefix {
                    'f' => { self.buf().find_char_forward(c); self.last_find = Some((c, true)); }
                    't' => {
                        let before = self.buf_ref().cursor;
                        if self.buf().find_char_forward(c) {
                            let col = self.buf_ref().cursor.1;
                            if col > before.1 { self.buf().cursor.1 = col - 1; }
                        }
                        self.last_find = Some((c, true));
                    }
                    'F' => { self.buf().find_char_backward(c); self.last_find = Some((c, false)); }
                    'T' => {
                        let before = self.buf_ref().cursor;
                        if self.buf().find_char_backward(c) {
                            let col = self.buf_ref().cursor.1;
                            if col + 1 < before.1 { self.buf().cursor.1 = col + 1; }
                        }
                        self.last_find = Some((c, false));
                    }
                    _ => {}
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_left(); }
                self.ensure_visible();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_down(); }
                self.ensure_visible();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_up(); }
                self.ensure_visible();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_right(); }
                self.ensure_visible();
            }
            KeyCode::Char('w') => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_word_forward(); }
                self.ensure_visible();
            }
            KeyCode::Char('b') => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_word_backward(); }
                self.ensure_visible();
            }
            KeyCode::Char('e') => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().move_word_end(); }
                self.ensure_visible();
            }
            KeyCode::Char('0') => { self.buf().move_line_start(); }
            KeyCode::Char('^') => { self.buf().move_first_non_blank(); }
            KeyCode::Char('$') => { self.buf().move_line_end(); }
            KeyCode::Char('g') => { self.pending_g = true; }
            KeyCode::Char('G') => {
                let n = self.take_count();
                if n > 0 { self.buf().goto_line(n); } else { self.buf().move_file_end(); }
                self.ensure_visible();
            }
            KeyCode::Char('{') => {
                self.buf().move_up();
                while self.buf_ref().cursor.0 > 0
                    && !self.buf_ref().current_line().trim().is_empty()
                { self.buf().move_up(); }
                self.ensure_visible();
            }
            KeyCode::Char('}') => {
                self.buf().move_down();
                let total = self.buf_ref().line_count();
                while self.buf_ref().cursor.0 + 1 < total
                    && !self.buf_ref().current_line().trim().is_empty()
                { self.buf().move_down(); }
                self.ensure_visible();
            }
            KeyCode::Char('%') => { self.buf().match_bracket(); self.ensure_visible(); }
            KeyCode::Char('i') => { self.mode = Mode::Insert; self.message.clear(); }
            KeyCode::Char('a') => {
                let len = self.buf_ref().line_chars(self.buf_ref().cursor.0);
                if self.buf_ref().cursor.1 < len { self.buf().move_right(); }
                self.mode = Mode::Insert; self.message.clear();
            }
            KeyCode::Char('I') => {
                self.buf().move_first_non_blank();
                self.mode = Mode::Insert; self.message.clear();
            }
            KeyCode::Char('A') => {
                let len = self.buf_ref().line_chars(self.buf_ref().cursor.0);
                self.buf().cursor.1 = len;
                self.mode = Mode::Insert; self.message.clear();
            }
            KeyCode::Char('o') => {
                let line = self.buf_ref().cursor.0;
                self.buf().cursor.1 = self.buf_ref().line_chars(line);
                self.buf().newline();
                self.mode = Mode::Insert; self.message.clear();
            }
            KeyCode::Char('O') => {
                let line = self.buf_ref().cursor.0;
                self.buf().cursor.1 = 0;
                self.buf().newline();
                self.buf().cursor.0 = line;
                self.mode = Mode::Insert; self.message.clear();
            }
            KeyCode::Char('x') => {
                let n = self.take_count().max(1);
                for _ in 0..n { self.buf().delete_char_at(); }
            }
            KeyCode::Char('d') => { self.pending_op = Some('d'); }
            KeyCode::Char('y') => { self.pending_op = Some('y'); }
            KeyCode::Char('c') => { self.pending_op = Some('c'); }
            KeyCode::Char('p') => {
                let text = self.clipboard.clone();
                if !text.is_empty() { self.buf().paste(&text, true); }
            }
            KeyCode::Char('P') => {
                let text = self.clipboard.clone();
                if !text.is_empty() { self.buf().paste(&text, false); }
            }
            KeyCode::Char('u') => { self.buf().undo(); }
            KeyCode::Char('>') => { self.buf().indent_line(); }
            KeyCode::Char('<') => { self.buf().outdent_line(); }
            KeyCode::Char('v') => {
                self.mode = Mode::Visual;
                let c = self.buf_ref().cursor;
                self.buf().visual_anchor = Some(c);
            }
            KeyCode::Char('V') => {
                self.mode = Mode::VisualLine;
                let c = self.buf_ref().cursor;
                self.buf().visual_anchor = Some(c);
            }
            KeyCode::Char(';') => {
                if let Some((c, fwd)) = self.last_find {
                    if fwd { self.buf().find_char_forward(c); }
                    else { self.buf().find_char_backward(c); }
                }
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_prefix = ':';
                self.command.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Command;
                self.command_prefix = '/';
                self.command.clear();
            }
            KeyCode::Char('n') => { self.search_next(1); }
            KeyCode::Char('N') => { self.search_next(-1); }
            KeyCode::Char('*') => { self.search_word(true); }
            KeyCode::Char('#') => { self.search_word(false); }
            KeyCode::Tab => { self.next_tab(); }
            KeyCode::BackTab => { self.prev_tab(); }
            KeyCode::Esc => {
                self.pending_count.clear();
                self.pending_op = None;
                self.pending_g = false;
                self.pending_f = None;
                self.search = None;
                self.search_hits.clear();
            }
            _ => { self.pending_count.clear(); }
        }
        self.pending_count.clear();
        self.buf().clamp_cursor();
        self.ensure_visible();
    }

    fn apply_operator(&mut self, op: char, motion: char) {
        match (op, motion) {
            ('d', 'd') => { let line = self.buf().delete_line(); self.clipboard = line; }
            ('y', 'y') => { let line = self.buf().yank_line(); self.clipboard = line; }
            ('d', 'w') => { let w = self.buf().delete_word(); self.clipboard = w; }
            ('y', 'w') => {
                let before = self.buf_ref().cursor;
                self.buf().move_word_forward();
                let after = self.buf_ref().cursor;
                if before.0 == after.0 {
                    let s: Vec<char> = self.buf_ref().current_line().chars().collect();
                    let a = before.1.min(s.len());
                    let b = after.1.min(s.len());
                    self.clipboard = s[a..b].iter().collect();
                    self.buf().cursor = before;
                }
            }
            ('c', 'w') => { let _ = self.buf().delete_word(); self.mode = Mode::Insert; self.message.clear(); }
            ('d', '$') => {
                let line = self.buf_ref().cursor.0;
                let col = self.buf_ref().cursor.1;
                let s: Vec<char> = self.buf_ref().line(line).chars().collect();
                self.clipboard = s[col.min(s.len())..].iter().collect();
                self.buf().lines[line] = s[..col.min(s.len())].iter().collect();
                self.buf().cursor.1 = self.buf_ref().line_chars(line).saturating_sub(1);
            }
            ('d', '0') => {
                let line = self.buf_ref().cursor.0;
                let col = self.buf_ref().cursor.1;
                let s: Vec<char> = self.buf_ref().line(line).chars().collect();
                self.clipboard = s[..col.min(s.len())].iter().collect();
                self.buf().lines[line] = s[col.min(s.len())..].iter().collect();
                self.buf().cursor.1 = 0;
            }
            _ => {}
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) {
        self.message.clear();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match key.code {
                KeyCode::Char('v') => {
                    let text = self.clipboard.clone();
                    if !text.is_empty() { self.buf().insert_str(&text); }
                    return;
                }
                KeyCode::Char('r') => { self.buf().redo(); return; }
                _ => {}
            }
        }

        if self.completion_active {
            match key.code {
                KeyCode::Esc => { self.completion_active = false; return; }
                KeyCode::Up => {
                    if self.completion_index > 0 { self.completion_index -= 1; }
                    else { self.completion_index = self.completion_items.len() - 1; }
                    return;
                }
                KeyCode::Down => {
                    self.completion_index = (self.completion_index + 1) % self.completion_items.len();
                    return;
                }
                KeyCode::Tab | KeyCode::Enter => { self.accept_completion(); return; }
                _ => { self.completion_active = false; }
            }
        }

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                if self.buf_ref().cursor.1 > 0 {
                    let len = self.buf_ref().line_chars(self.buf_ref().cursor.0);
                    if self.buf_ref().cursor.1 >= len {
                        self.buf().cursor.1 = len.saturating_sub(1);
                    } else {
                        self.buf().cursor.1 = self.buf().cursor.1.saturating_sub(1);
                    }
                }
            }
            KeyCode::Enter => { self.auto_indent_enter(); }
            KeyCode::Backspace => {
                if !self.try_auto_pair_backspace() { self.buf().backspace(); }
            }
            KeyCode::Delete => { self.buf().delete_char_at(); }
            KeyCode::Tab => { self.buf().insert_str("    "); }
            KeyCode::Left => { self.buf().move_left(); }
            KeyCode::Right => { self.buf().move_right(); }
            KeyCode::Up => { self.buf().move_up(); }
            KeyCode::Down => { self.buf().move_down(); }
            KeyCode::Home => { self.buf().move_line_start(); }
            KeyCode::End => {
                let len = self.buf_ref().line_chars(self.buf_ref().cursor.0);
                self.buf().cursor.1 = len;
            }
            KeyCode::Char(c) => {
                self.insert_with_pairs(c);
                if c.is_alphanumeric() || c == '_' { self.compute_completions(); }
            }
            _ => {}
        }
        self.buf().clamp_cursor();
        self.ensure_visible();
    }

    fn insert_with_pairs(&mut self, c: char) {
        let close = match c {
            '(' => Some(')'), '{' => Some('}'),
            '[' => Some(']'), '"' => Some('"'),
            '\'' => Some('\''), _ => None,
        };
        if let Some(cl) = close {
            self.buf().insert_char(c);
            self.buf().insert_char(cl);
            self.buf().cursor.1 = self.buf().cursor.1.saturating_sub(1);
        } else {
            self.buf().insert_char(c);
        }
    }

    fn try_auto_pair_backspace(&mut self) -> bool {
        let (l, c) = self.buf_ref().cursor;
        let chars: Vec<char> = self.buf_ref().line(l).chars().collect();
        if c > 0 && c < chars.len() {
            let prev = chars[c - 1];
            let cur = chars[c];
            let pair = matches!(
                (prev, cur),
                ('(', ')') | ('{', '}') | ('[', ']') | ('"', '"') | ('\'', '\'')
            );
            if pair {
                self.buf().delete_char_at();
                self.buf().backspace();
                return true;
            }
        }
        false
    }

    fn auto_indent_enter(&mut self) {
        let line = self.buf_ref().current_line().to_string();
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let cursor_col = self.buf_ref().cursor.1;
        let chars: Vec<char> = line.chars().collect();
        let before_cur: String = chars[..cursor_col.min(chars.len())].iter().collect();
        let trim = before_cur.trim_end();
        let last = trim.chars().last();
        let extra = if matches!(last, Some('{') | Some('(') | Some('[')) { "    " } else { "" };
        self.buf().newline();
        let new_indent = format!("{}{}", indent, extra);
        if !new_indent.is_empty() { self.buf().insert_str(&new_indent); }
    }

    fn compute_completions(&mut self) {
        let cursor = self.buf_ref().cursor;
        let line = self.buf_ref().current_line().to_string();
        let chars: Vec<char> = line.chars().collect();
        let mut start = cursor.1;
        while start > 0 {
            let c = chars[start - 1];
            if c.is_alphanumeric() || c == '_' { start -= 1; } else { break; }
        }
        let prefix: String = chars[start..cursor.1.min(chars.len())].iter().collect();
        if prefix.len() < 2 { self.completion_active = false; return; }

        let mut set: std::collections::HashSet<String> = std::collections::HashSet::new();
        for line in &self.buf_ref().lines {
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if word.len() > prefix.len() && word.starts_with(&prefix) && word != prefix {
                    set.insert(word.to_string());
                    if set.len() > 64 { break; }
                }
            }
            if set.len() > 64 { break; }
        }
        if set.is_empty() { self.completion_active = false; return; }

        let mut items: Vec<String> = set.into_iter().collect();
        items.sort();
        self.completion_items = items;
        self.completion_index = 0;
        self.completion_prefix = prefix;
        self.completion_active = true;
    }

    fn accept_completion(&mut self) {
        if !self.completion_active { return; }
        let item = match self.completion_items.get(self.completion_index) {
            Some(s) => s.clone(), None => return,
        };
        let n = self.completion_prefix.chars().count();
        self.buf().replace_before_cursor(n, &item);
        self.completion_active = false;
    }

    fn handle_visual(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match key.code {
                KeyCode::Char('c') => {
                    let text = self.buf().yank_selection();
                    self.system_copy(&text);
                    self.mode = Mode::Normal;
                    self.message = "copied".into();
                    return;
                }
                KeyCode::Char('x') => {
                    let text = self.buf().delete_selection();
                    self.system_copy(&text);
                    self.mode = Mode::Normal;
                    self.message = "cut".into();
                    return;
                }
                KeyCode::Char('a') => {
                    let last = self.buf_ref().line_count().saturating_sub(1);
                    let last_len = self.buf_ref().line_chars(last);
                    self.buf().visual_anchor = Some((0, 0));
                    self.buf().cursor = (last, last_len);
                    return;
                }
                KeyCode::Char('v') => {
                    let text = self.clipboard.clone();
                    if !text.is_empty() { self.buf().paste(&text, true); }
                    self.mode = Mode::Normal;
                    return;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => { self.mode = Mode::Normal; self.buf().visual_anchor = None; }
            KeyCode::Char('h') | KeyCode::Left => { self.buf().move_left(); self.ensure_visible(); }
            KeyCode::Char('j') | KeyCode::Down => { self.buf().move_down(); self.ensure_visible(); }
            KeyCode::Char('k') | KeyCode::Up => { self.buf().move_up(); self.ensure_visible(); }
            KeyCode::Char('l') | KeyCode::Right => { self.buf().move_right(); self.ensure_visible(); }
            KeyCode::Char('w') => { self.buf().move_word_forward(); }
            KeyCode::Char('b') => { self.buf().move_word_backward(); }
            KeyCode::Char('e') => { self.buf().move_word_end(); }
            KeyCode::Char('0') => { self.buf().move_line_start(); }
            KeyCode::Char('$') => {
                let len = self.buf_ref().line_chars(self.buf_ref().cursor.0);
                self.buf().cursor.1 = if len > 0 { len - 1 } else { 0 };
            }
            KeyCode::Char('g') => { self.buf().move_file_start(); self.ensure_visible(); }
            KeyCode::Char('G') => { self.buf().move_file_end(); self.ensure_visible(); }
            KeyCode::Char('y') => {
                let text = self.buf().yank_selection();
                self.system_copy(&text);
                self.mode = Mode::Normal;
            }
            KeyCode::Char('d') | KeyCode::Char('x') => {
                let text = self.buf().delete_selection();
                self.system_copy(&text);
                self.mode = Mode::Normal;
            }
            KeyCode::Char('c') => {
                let text = self.buf().delete_selection();
                self.system_copy(&text);
                self.mode = Mode::Insert;
                self.message.clear();
            }
            KeyCode::Char('>') => {
                if let Some(((a, _), (b, _))) = self.buf_ref().visual_range() {
                    for i in a..=b { self.buf().cursor.0 = i; self.buf().indent_line(); }
                }
                self.mode = Mode::Normal;
                self.buf().visual_anchor = None;
            }
            KeyCode::Char('<') => {
                if let Some(((a, _), (b, _))) = self.buf_ref().visual_range() {
                    for i in a..=b { self.buf().cursor.0 = i; self.buf().outdent_line(); }
                }
                self.mode = Mode::Normal;
                self.buf().visual_anchor = None;
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => { self.mode = Mode::Normal; self.command.clear(); }
            KeyCode::Enter => {
                let cmd = self.command.clone();
                let prefix = self.command_prefix;
                self.command.clear();
                self.mode = Mode::Normal;
                if prefix == '/' { self.set_search(&cmd); }
                else { self.execute_command(&cmd); }
            }
            KeyCode::Backspace => { self.command.pop(); }
            KeyCode::Char(c) => { self.command.push(c); }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() { return; }
        if let Ok(n) = cmd.parse::<usize>() {
            self.buf().goto_line(n);
            self.ensure_visible();
            return;
        }
        match cmd {
            "w" => match self.buf().save() {
                Ok(_) => self.message = "written".into(),
                Err(e) => self.message = format!("write error: {}", e),
            },
            "q" => {
                if self.buf_ref().modified {
                    self.message = "no write since last change (add ! to override)".into();
                } else { self.close_current_tab(); }
            }
            "q!" => { self.close_current_tab(); }
            "wq" | "x" => { let _ = self.buf().save(); self.close_current_tab(); }
            "run" => { self.run_file(); }
            "reload" => {
                self.config.reload();
                self.theme = Theme::by_name(&self.config.theme);
                self.config.apply_color_overrides(&mut self.theme);
                self.tree.width = self.config.tree_width;
                if let Ok(meta) = std::fs::metadata(&self.config.path) {
                    if let Ok(mtime) = meta.modified() {
                        self.config_mtime = Some(mtime);
                    }
                }
                self.message = "config reloaded".into();
            }
            "theme" | "themes" => { self.open_theme_picker(); }
            "new" => { self.open_input(InputKind::NewFile, ""); }
            "goto" => { self.open_input(InputKind::GotoLine, ""); }
            "term" | "terminal" => { self.open_bottom_term(None); }
            "sterm" => { self.open_side_term(None); }
            _ => {
                if let Some(rest) = cmd.strip_prefix("w ") {
                    let p = PathBuf::from(rest.trim());
                    match self.buf().save_as(p) {
                        Ok(_) => self.message = "written".into(),
                        Err(e) => self.message = format!("write error: {}", e),
                    }
                } else if let Some(rest) = cmd.strip_prefix("e ") {
                    self.open_file(PathBuf::from(rest.trim()));
                } else if let Some(name) = cmd.strip_prefix("set theme ") {
                    self.preview_theme(name.trim());
                    self.save_config_now();
                    self.message = format!("theme: {}", name.trim());
                } else if cmd.starts_with("%s/") || cmd.starts_with("s/") {
                    self.substitute(cmd);
                } else {
                    self.message = format!("unknown command: {}", cmd);
                }
            }
        }
    }

    fn substitute(&mut self, cmd: &str) {
        let global_all = cmd.starts_with('%');
        let body = if global_all { cmd.trim_start_matches("%s/") }
                   else { cmd.trim_start_matches("s/") };
        let last_slash = match body.rfind('/') {
            Some(i) => i,
            None => { self.message = "bad substitute".into(); return; }
        };
        let (rest, _flags) = body.split_at(last_slash);
        let mid = match rest.find('/') {
            Some(i) => i,
            None => { self.message = "bad substitute".into(); return; }
        };
        let from = &rest[..mid];
        let to = &rest[mid + 1..];
        let n = if global_all { self.buf().replace_all(from, to) }
                else { self.buf().replace_line(from, to) };
        self.message = format!("{} substitutions", n);
    }

    fn set_search(&mut self, pattern: &str) {
        if pattern.is_empty() {
            self.search = None;
            self.search_hits.clear();
            return;
        }
        self.search = Some(pattern.to_string());
        self.search_hits = self.buf_ref().find_all(pattern);
        self.search_index = 0;
        if !self.search_hits.is_empty() { self.jump_to_hit(0); }
        self.message = format!("[{}/{}]", 1, self.search_hits.len().max(1));
    }

    fn search_next(&mut self, dir: i32) {
        if self.search_hits.is_empty() { return; }
        let len = self.search_hits.len() as i32;
        let mut idx = self.search_index as i32 + dir;
        if idx < 0 { idx = len - 1; }
        if idx >= len { idx = 0; }
        self.search_index = idx as usize;
        self.jump_to_hit(self.search_index);
        self.message = format!("[{}/{}]", self.search_index + 1, len);
    }

    fn jump_to_hit(&mut self, idx: usize) {
        if let Some(hit) = self.search_hits.get(idx).copied() {
            self.buf().goto_hit(hit);
            self.ensure_visible();
        }
    }

    fn search_word(&mut self, forward: bool) {
        if let Some(w) = self.buf_ref().word_under_cursor() {
            self.set_search(&w);
            if !forward && self.search_hits.len() > 1 { self.search_next(-1); }
        }
    }

    fn handle_leader_key(&mut self, key: KeyEvent) {
        self.leader = false;
        match key.code {
            KeyCode::Char('w') => match self.buf().save() {
                Ok(_) => self.message = "written".into(),
                Err(e) => self.message = format!("write error: {}", e),
            },
            KeyCode::Char('q') | KeyCode::Char('x') => { self.close_current_tab(); }
            KeyCode::Char('Q') => { self.should_quit = true; }

            KeyCode::Char('a') => {
                let last = self.buf_ref().line_count().saturating_sub(1);
                let last_len = self.buf_ref().line_chars(last);
                self.buf().visual_anchor = Some((0, 0));
                self.buf().cursor = (last, last_len);
                self.mode = Mode::Visual;
                self.message = "select all".into();
            }
            KeyCode::Char('c') => {
                let text = if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                    self.buf().yank_selection()
                } else {
                    self.buf_ref().current_line().to_string()
                };
                self.system_copy(&text);
                self.message = "copied".into();
                if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char('r') => { self.run_file(); }

            KeyCode::Char('s') => {
                if !self.tree.visible {
                    self.tree.visible = true;
                    self.config.tree = true;
                    self.tree_focus = true;
                    self.active_pane = Pane::Tree;
                    self.save_config_now();
                    self.message = "tree".into();
                } else if self.tree_focus {
                    self.tree_focus = false;
                    self.active_pane = Pane::Editor;
                    self.message = "editor".into();
                } else {
                    self.tree_focus = true;
                    self.active_pane = Pane::Tree;
                    self.message = "tree".into();
                }
            }

            KeyCode::Char('e') => {
                self.tree.visible = !self.tree.visible;
                self.config.tree = self.tree.visible;
                self.tree_focus = self.tree.visible;
                self.active_pane = if self.tree.visible { Pane::Tree } else { Pane::Editor };
                self.save_config_now();
            }
            KeyCode::Char('f') => { let r = self.tree.root.clone(); self.finder.open_files(&r); }
            KeyCode::Char('F') => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                self.finder.open_files(&PathBuf::from(home));
            }
            KeyCode::Char('g') => { let r = self.tree.root.clone(); self.finder.open_grep(&r); }
            KeyCode::Char('n') => { self.open_input(InputKind::NewFile, ""); }
            KeyCode::Char('t') => { self.open_theme_picker(); }
            KeyCode::Char('h') => { self.show_help = true; }
            KeyCode::Char('l') => { self.open_input(InputKind::GotoLine, ""); }
            KeyCode::Char('j') => {
                let h = self.config.term_bottom_height;
                if let Some(t) = &mut self.bottom_term {
                    t.visible = !t.visible;
                    if t.visible { self.active_pane = Pane::BottomTerm; }
                    else if self.active_pane == Pane::BottomTerm { self.active_pane = Pane::Editor; }
                } else {
                    self.ensure_bottom_term(h, 100);
                    if let Some(t) = &mut self.bottom_term { t.visible = true; }
                    self.active_pane = Pane::BottomTerm;
                }
            }
            KeyCode::Char('v') => {
                let w = self.config.term_side_width;
                let h = self.area_height.saturating_sub(4).max(8);
                if let Some(t) = &mut self.side_term {
                    t.visible = !t.visible;
                    if t.visible { self.active_pane = Pane::SideTerm; }
                    else if self.active_pane == Pane::SideTerm { self.active_pane = Pane::Editor; }
                } else {
                    self.ensure_side_term(h, w);
                    if let Some(t) = &mut self.side_term { t.visible = true; }
                    self.active_pane = Pane::SideTerm;
                }
            }
            KeyCode::Esc => {}
            _ => {}
        }
    }

    fn handle_finder_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => { self.finder.close(); }
            KeyCode::Enter => { self.finder_open_selected(); }
            KeyCode::Up => { self.finder.move_up(); }
            KeyCode::Down => { self.finder.move_down(); }
            KeyCode::Backspace => { self.finder.query.pop(); self.refresh_finder(); }
            KeyCode::Char(c) => { self.finder.query.push(c); self.refresh_finder(); }
            _ => {}
        }
    }

    fn refresh_finder(&mut self) {
        if self.finder.kind == FinderKind::File {
            self.finder.update_files();
        } else {
            let root = self.tree.root.clone();
            self.finder.update_grep(&root);
        }
    }

    fn finder_open_selected(&mut self) {
        match self.finder.kind {
            FinderKind::File => {
                if let Some(p) = self.finder.file_results.get(self.finder.selected).cloned() {
                    self.finder.close();
                    self.open_file(p);
                }
            }
            FinderKind::Grep => {
                if let Some(hit) = self.finder.grep_results.get(self.finder.selected) {
                    let p = hit.path.clone();
                    let line = hit.line;
                    self.finder.close();
                    self.open_file(p);
                    self.buf().goto_line(line);
                    self.ensure_visible();
                }
            }
        }
    }

    pub fn open_file(&mut self, path: PathBuf) {
        for (i, b) in self.buffers.iter().enumerate() {
            if b.path.as_ref() == Some(&path) {
                self.current = i;
                self.dashboard_dismissed = true;
                self.active_pane = Pane::Editor;
                return;
            }
        }
        match Buffer::from_file(path.clone()) {
            Ok(b) => {
                self.install_buffer(b);
                self.active_pane = Pane::Editor;
            }
            Err(e) => self.message = format!("open error: {}", e),
        }
    }

    fn close_current_tab(&mut self) {
        if self.buffers.len() == 1 { self.should_quit = true; return; }
        self.buffers.remove(self.current);
        if self.current >= self.buffers.len() { self.current = self.buffers.len() - 1; }
    }

    fn next_tab(&mut self) {
        if self.buffers.is_empty() { return; }
        self.current = (self.current + 1) % self.buffers.len();
        self.dashboard_dismissed = true;
    }

    fn prev_tab(&mut self) {
        if self.buffers.is_empty() { return; }
        if self.current == 0 { self.current = self.buffers.len() - 1; }
        else { self.current -= 1; }
        self.dashboard_dismissed = true;
    }

    pub fn text_gutter_width(&self) -> u16 {
        let total = self.buf_ref().line_count();
        let num_width = total.to_string().len().max(3) as u16;
        num_width + 4
    }

    // ================= MOUSE =================
    pub fn handle_mouse(&mut self, m: MouseEvent) {
        if !self.config.mouse { return; }

        let in_rect = |x: u16, y: u16, r: Rect| -> bool {
            r.width > 0 && r.height > 0
                && x >= r.x && x < r.x + r.width
                && y >= r.y && y < r.y + r.height
        };

        // ── Popups first ──
        if self.theme_picker_visible {
            self.handle_theme_picker_mouse(m, &in_rect);
            return;
        }
        if self.finder.visible {
            self.handle_finder_mouse(m, &in_rect);
            return;
        }
        if self.show_help {
            if matches!(m.kind, MouseEventKind::Down(_)) {
                self.show_help = false;
            }
            return;
        }
        if self.confirm_delete.is_some() { return; }

        let in_tree = self.tree.visible && in_rect(m.column, m.row, self.last_tree_area);
        let in_editor = in_rect(m.column, m.row, self.last_text_area);
        let in_tabs = in_rect(m.column, m.row, self.last_tabs_area);
        let in_side_t = self.side_term.as_ref().map(|t| t.visible && in_rect(m.column, m.row, self.last_side_term_area)).unwrap_or(false);
        let in_bot_t = self.bottom_term.as_ref().map(|t| t.visible && in_rect(m.column, m.row, self.last_bottom_term_area)).unwrap_or(false);

        match m.kind {
            MouseEventKind::ScrollDown => {
                if in_tree {
                    for _ in 0..3 { self.tree.move_down(); }
                    self.sync_tree_scroll();
                } else if in_side_t {
                    if let Some(t) = &mut self.side_term { t.scroll_down_viewport(); }
                } else if in_bot_t {
                    if let Some(t) = &mut self.bottom_term { t.scroll_down_viewport(); }
                } else if in_editor {
                    let max_scroll = self.buf_ref().line_count().saturating_sub(1) as u16;
                    let cur = self.buf_ref().scroll;
                    self.buf().scroll = (cur + 3).min(max_scroll);
                }
            }
            MouseEventKind::ScrollUp => {
                if in_tree {
                    for _ in 0..3 { self.tree.move_up(); }
                    self.sync_tree_scroll();
                } else if in_side_t {
                    if let Some(t) = &mut self.side_term { t.scroll_up_viewport(); }
                } else if in_bot_t {
                    if let Some(t) = &mut self.bottom_term { t.scroll_up_viewport(); }
                } else if in_editor {
                    let cur = self.buf_ref().scroll;
                    self.buf().scroll = cur.saturating_sub(3);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if in_tree {
                    self.tree_focus = true;
                    self.active_pane = Pane::Tree;
                    let rel_y = (m.row - self.last_tree_area.y).saturating_sub(1) as usize;
                    let idx = self.tree.scroll as usize + rel_y;
                    if idx < self.tree.entries.len() {
                        let now = Instant::now();
                        let is_double = matches!(
                            self.last_click,
                            Some((x, y, t))
                                if x == m.column && y == m.row
                                && now.duration_since(t).as_millis() < 400
                        );
                        self.tree.selected = idx;
                        if is_double {
                            if self.tree.selected_is_dir() {
                                self.tree.toggle_selected();
                                self.sync_tree_scroll();
                            } else if let Some(p) = self.tree.selected_path() {
                                self.open_file(p);
                            }
                        }
                        self.last_click = Some((m.column, m.row, now));
                    }
                } else if in_side_t {
                    self.active_pane = Pane::SideTerm;
                    self.tree_focus = false;
                } else if in_bot_t {
                    self.active_pane = Pane::BottomTerm;
                    self.tree_focus = false;
                } else if in_editor {
                    self.active_pane = Pane::Editor;
                    self.tree_focus = false;
                    let gutter = self.text_gutter_width();
                    let rel_y = (m.row - self.last_text_area.y) as usize;
                    let rel_x = (m.column
                        .saturating_sub(self.last_text_area.x)
                        .saturating_sub(gutter)) as usize;
                    let line = (self.buf_ref().scroll as usize + rel_y)
                        .min(self.buf_ref().line_count().saturating_sub(1));
                    let col = rel_x.min(self.buf_ref().line_chars(line));
                    self.buf().cursor = (line, col);
                    self.buf().preferred_col = col;
                    self.buf().visual_anchor = None;
                    if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                        self.mode = Mode::Normal;
                    }
                    self.mouse_dragging = true;
                } else if in_tabs {
                    let rel_x = (m.column - self.last_tabs_area.x) as usize;
                    let mut acc = 0usize;
                    for (i, b) in self.buffers.iter().enumerate() {
                        let mut name = b.filename();
                        if b.modified { name.push_str(" ●"); }
                        let w = name.chars().count() + 4;
                        if rel_x < acc + w { self.current = i; break; }
                        acc += w;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.mouse_dragging && in_editor {
                    if self.mode == Mode::Normal {
                        self.mode = Mode::Visual;
                        let c = self.buf_ref().cursor;
                        self.buf().visual_anchor = Some(c);
                    }
                    let gutter = self.text_gutter_width();
                    let rel_y = (m.row - self.last_text_area.y) as usize;
                    let rel_x = (m.column
                        .saturating_sub(self.last_text_area.x)
                        .saturating_sub(gutter)) as usize;
                    let line = (self.buf_ref().scroll as usize + rel_y)
                        .min(self.buf_ref().line_count().saturating_sub(1));
                    let col = rel_x.min(self.buf_ref().line_chars(line));
                    self.buf().cursor = (line, col);
                    self.buf().preferred_col = col;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => { self.mouse_dragging = false; }
            MouseEventKind::Down(MouseButton::Right) => {
                if in_tree {
                    if self.tree.selected_is_dir() { self.tree.toggle_selected(); }
                }
            }
            _ => {}
        }
    }

    // ==================== MOUSE HELPERS ====================
    fn handle_finder_mouse<F>(&mut self, m: MouseEvent, in_rect: &F)
    where F: Fn(u16, u16, Rect) -> bool,
    {
        let list_area = self.last_finder_list_area;
        let in_list = in_rect(m.column, m.row, list_area);
        let total = match self.finder.kind {
            FinderKind::File => self.finder.file_results.len(),
            FinderKind::Grep => self.finder.grep_results.len(),
        };

        match m.kind {
            MouseEventKind::ScrollDown => {
                self.finder.move_down();
            }
            MouseEventKind::ScrollUp => {
                self.finder.move_up();
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if in_list && total > 0 {
                    let rel_y = (m.row - list_area.y) as usize;
                    let clicked = rel_y.min(total.saturating_sub(1));
                    let now = Instant::now();
                    let is_double = matches!(
                        self.last_click,
                        Some((x, y, t))
                            if x == m.column && y == m.row
                            && now.duration_since(t).as_millis() < 400
                    );
                    self.finder.selected = clicked;
                    self.last_click = Some((m.column, m.row, now));
                    if is_double {
                        self.finder_open_selected();
                    }
                } else {
                    self.finder.close();
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.finder.close();
            }
            _ => {}
        }
    }

    fn handle_theme_picker_mouse<F>(&mut self, m: MouseEvent, in_rect: &F)
    where F: Fn(u16, u16, Rect) -> bool,
    {
        let list_area = self.last_theme_list_area;
        let in_list = in_rect(m.column, m.row, list_area);
        let total = THEME_NAMES.len();

        match m.kind {
            MouseEventKind::ScrollDown => {
                if self.theme_picker_index + 1 < total {
                    self.theme_picker_index += 1;
                }
                let name = THEME_NAMES[self.theme_picker_index];
                self.preview_theme(name);
            }
            MouseEventKind::ScrollUp => {
                if self.theme_picker_index > 0 {
                    self.theme_picker_index -= 1;
                }
                let name = THEME_NAMES[self.theme_picker_index];
                self.preview_theme(name);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if in_list && total > 0 {
                    let rel_y = (m.row - list_area.y) as usize;
                    let view_h = list_area.height as usize;
                    let sel = self.theme_picker_index;
                    let scroll = if sel >= view_h { sel + 1 - view_h } else { 0 };
                    let idx = (scroll + rel_y).min(total.saturating_sub(1));

                    let now = Instant::now();
                    let is_double = matches!(
                        self.last_click,
                        Some((x, y, t))
                            if x == m.column && y == m.row
                            && now.duration_since(t).as_millis() < 400
                    );
                    self.theme_picker_index = idx;
                    self.preview_theme(THEME_NAMES[idx]);
                    self.last_click = Some((m.column, m.row, now));

                    if is_double {
                        self.save_config_now();
                        self.theme_picker_visible = false;
                        self.message = format!("theme: {}", THEME_NAMES[idx]);
                    }
                } else {
                    self.theme_picker_visible = false;
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.theme_picker_visible = false;
            }
            _ => {}
        }
    }

    fn take_count(&mut self) -> usize {
        let s = std::mem::take(&mut self.pending_count);
        s.parse::<usize>().unwrap_or(0)
    }

    pub fn ensure_visible(&mut self) {
        let total = self.buf_ref().line_count();
        if total == 0 { self.buf().scroll = 0; return; }
        if self.buf_ref().cursor.0 >= total {
            self.buf().cursor.0 = total - 1;
        }
        let max_line = self.buf_ref().line_chars(self.buf_ref().cursor.0);
        if self.buf_ref().cursor.1 > max_line {
            self.buf().cursor.1 = max_line;
        }
        let view_h = (self.editor_view_h as usize).max(1);
        let max_scroll = total.saturating_sub(view_h).min(total.saturating_sub(1));
        let row = self.buf_ref().cursor.0;
        let mut scroll = self.buf_ref().scroll as usize;
        if scroll > max_scroll { scroll = max_scroll; }
        if row < scroll { scroll = row; }
        else if row >= scroll + view_h { scroll = row + 1 - view_h; }
        if scroll > max_scroll { scroll = max_scroll; }
        self.buf().scroll = scroll as u16;
    }

    pub fn mode_str(&self) -> &'static str {
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
            Mode::Command => "COMMAND",
        }
    }

    pub fn elapsed_str(&self) -> String {
        let d = self.start_time.elapsed();
        let s = d.as_secs();
        if s < 60 { format!("{}s", s) }
        else if s < 3600 { format!("{}m{}s", s / 60, s % 60) }
        else { format!("{}h{}m", s / 3600, (s % 3600) / 60) }
    }
}

// ---------- helpers ----------

fn keybind_matches(binding: &str, key: &KeyEvent) -> bool {
    let b = binding.to_lowercase();
    if let Some(rest) = b.strip_prefix("ctrl+") {
        if !key.modifiers.contains(KeyModifiers::CONTROL) { return false; }
        return match (rest, key.code) {
            (s, KeyCode::Char(c)) if s.chars().count() == 1 => {
                s.chars().next() == Some(c.to_ascii_lowercase())
            }
            _ => false,
        };
    }
    if let Some(rest) = b.strip_prefix("alt+") {
        if !key.modifiers.contains(KeyModifiers::ALT) { return false; }
        return match (rest, key.code) {
            (s, KeyCode::Char(c)) if s.chars().count() == 1 => {
                s.chars().next() == Some(c.to_ascii_lowercase())
            }
            _ => false,
        };
    }
    if let Some(nstr) = b.strip_prefix('f') {
        if let Ok(n) = nstr.parse::<u8>() {
            if let KeyCode::F(k) = key.code { return k == n; }
            return false;
        }
    }
    if b == "space" { return matches!(key.code, KeyCode::Char(' ')); }
    if b.chars().count() == 1 {
        if let KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                return c.to_ascii_lowercase() == b.chars().next().unwrap();
            }
        }
    }
    false
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "/._-~".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn key_event_to_bytes(k: &KeyEvent) -> Option<Vec<u8>> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let mut prefix: Vec<u8> = Vec::new();
    if alt { prefix.push(0x1b); }
    match k.code {
        KeyCode::Char(c) => {
            if ctrl {
                let b = (c as u8) & 0x1f;
                prefix.push(b);
            } else {
                let mut buf = [0u8; 4];
                prefix.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            Some(prefix)
        }
        KeyCode::Enter => { prefix.push(b'\r'); Some(prefix) }
        KeyCode::Backspace => { prefix.push(0x7f); Some(prefix) }
        KeyCode::Tab => { prefix.push(b'\t'); Some(prefix) }
        KeyCode::Esc => { prefix.push(0x1b); Some(prefix) }
        KeyCode::Up => { prefix.extend_from_slice(b"\x1b[A"); Some(prefix) }
        KeyCode::Down => { prefix.extend_from_slice(b"\x1b[B"); Some(prefix) }
        KeyCode::Right => { prefix.extend_from_slice(b"\x1b[C"); Some(prefix) }
        KeyCode::Left => { prefix.extend_from_slice(b"\x1b[D"); Some(prefix) }
        KeyCode::Home => { prefix.extend_from_slice(b"\x1b[H"); Some(prefix) }
        KeyCode::End => { prefix.extend_from_slice(b"\x1b[F"); Some(prefix) }
        KeyCode::PageUp => { prefix.extend_from_slice(b"\x1b[5~"); Some(prefix) }
        KeyCode::PageDown => { prefix.extend_from_slice(b"\x1b[6~"); Some(prefix) }
        KeyCode::Delete => { prefix.extend_from_slice(b"\x1b[3~"); Some(prefix) }
        KeyCode::Insert => { prefix.extend_from_slice(b"\x1b[2~"); Some(prefix) }
        _ => None,
    }
}
