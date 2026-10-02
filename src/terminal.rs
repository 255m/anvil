use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use ratatui::style::Color;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use vte::{Params, Parser, Perform};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TermKind { Bottom, Left }

#[derive(Clone)]
pub struct TermCell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

impl Default for TermCell {
    fn default() -> Self {
        Self { ch: ' ', fg: Color::Reset, bg: Color::Reset, bold: false }
    }
}

pub struct Terminal {
    pub kind: TermKind,
    pub visible: bool,
    pub width: usize,
    pub height: usize,
    pub grid: Vec<TermCell>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    pub exited: bool,

    parser: Option<Parser>,
    cur_fg: Color,
    cur_bg: Color,
    cur_bold: bool,
    saved: (usize, usize),

    /// ردود على queries من shell (يجب إرسالها بعد كل pump)
    pending_replies: Vec<u8>,

    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    rx: Receiver<Vec<u8>>,
}

impl Terminal {
    pub fn spawn(kind: TermKind, shell: Option<&str>, rows: u16, cols: u16) -> std::io::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let shell_path = shell
            .map(|s| s.to_string())
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        let mut cmd = CommandBuilder::new(shell_path);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair.slave
            .spawn_command(cmd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        drop(pair.slave);

        let master = pair.master;
        let writer = master
            .take_writer()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let mut reader = master
            .try_clone_reader()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let (tx, rx) = channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut buf = [0u8; 65536];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() { break; }
                    }
                }
            }
        });

        let w = cols as usize;
        let h = rows as usize;
        Ok(Self {
            kind, visible: false, width: w, height: h,
            grid: vec![TermCell::default(); w * h],
            cursor_x: 0, cursor_y: 0, cursor_visible: true, exited: false,
            parser: Some(Parser::new()),
            cur_fg: Color::Reset, cur_bg: Color::Reset, cur_bold: false,
            saved: (0, 0),
            pending_replies: Vec::new(),
            master, writer, child, rx,
        })
    }

    fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    fn clear_grid(&mut self) {
        let bg = self.cur_bg;
        for c in &mut self.grid { *c = TermCell { ch: ' ', fg: self.cur_fg, bg, bold: false }; }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let w = cols as usize;
        let h = rows as usize;
        if w == self.width && h == self.height { return; }
        self.width = w;
        self.height = h;
        self.grid = vec![TermCell::default(); w * h];
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
    }

    pub fn pump(&mut self) {
        if self.exited { return; }

        if let Ok(Some(_)) = self.child.try_wait() {
            self.exited = true;
            return;
        }

        let mut chunks: Vec<Vec<u8>> = Vec::new();
        for _ in 0..512 {
            match self.rx.try_recv() {
                Ok(data) => chunks.push(data),
                Err(_) => break,
            }
        }
        for data in chunks {
            let mut parser = self.parser.take().unwrap_or_else(Parser::new);
            for b in data.iter() {
                parser.advance(self, *b);
            }
            self.parser = Some(parser);
        }

        // إرسال ردود queries المتراكمة
        if !self.pending_replies.is_empty() {
            let replies = std::mem::take(&mut self.pending_replies);
            if self.writer.write_all(&replies).is_err() {
                self.exited = true;
                return;
            }
            let _ = self.writer.flush();
        }
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) {
        if self.exited { return; }
        if self.writer.write_all(bytes).is_err() {
            self.exited = true;
            return;
        }
        if self.writer.flush().is_err() {
            self.exited = true;
        }
    }

    pub fn send_str(&mut self, s: &str) { self.send_bytes(s.as_bytes()); }

    pub fn scroll_up_viewport(&mut self) { self.send_bytes(b"\x1b[5~"); }
    pub fn scroll_down_viewport(&mut self) { self.send_bytes(b"\x1b[6~"); }

    /// استخراج سطر نص من grid (للنسخ عند التحديد)
    pub fn text_in_range(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let (mut c1, mut r1) = start;
        let (mut c2, mut r2) = end;
        if (r1, c1) > (r2, c2) {
            std::mem::swap(&mut c1, &mut c2);
            std::mem::swap(&mut r1, &mut r2);
        }
        let mut out = String::new();
        if r1 == r2 {
            for x in c1..=c2 {
                if x < self.width && r1 < self.height {
                    out.push(self.grid[r1 * self.width + x].ch);
                }
            }
        } else {
            for x in c1..self.width {
                out.push(self.grid[r1 * self.width + x].ch);
            }
            out.push('\n');
            for y in (r1 + 1)..r2 {
                for x in 0..self.width {
                    out.push(self.grid[y * self.width + x].ch);
                }
                out.push('\n');
            }
            for x in 0..=c2.min(self.width.saturating_sub(1)) {
                out.push(self.grid[r2 * self.width + x].ch);
            }
        }
        out
    }

    fn put(&mut self, ch: char) {
        if self.cursor_x >= self.width {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.height { self.scroll_up(); }
        }
        if self.cursor_y >= self.height { self.scroll_up(); }
        let idx = self.idx(self.cursor_x, self.cursor_y);
        if idx < self.grid.len() {
            self.grid[idx] = TermCell {
                ch, fg: self.cur_fg, bg: self.cur_bg, bold: self.cur_bold,
            };
        }
        self.cursor_x += 1;
    }

    fn scroll_up(&mut self) {
        let w = self.width;
        self.grid.drain(0..w);
        self.grid.resize(w * self.height, TermCell::default());
        self.cursor_y = self.height.saturating_sub(1);
    }
}

fn ansi_color(i: u8) -> Color {
    match i {
        0 => Color::Rgb(40, 40, 40),
        1 => Color::Rgb(204, 60, 60),
        2 => Color::Rgb(80, 180, 80),
        3 => Color::Rgb(220, 180, 60),
        4 => Color::Rgb(80, 120, 220),
        5 => Color::Rgb(200, 100, 200),
        6 => Color::Rgb(80, 190, 200),
        7 => Color::Rgb(200, 200, 200),
        _ => Color::Reset,
    }
}

fn ansi_color_bright(i: u8) -> Color {
    match i {
        0 => Color::Rgb(90, 90, 90),
        1 => Color::Rgb(240, 90, 90),
        2 => Color::Rgb(120, 220, 120),
        3 => Color::Rgb(250, 220, 100),
        4 => Color::Rgb(120, 160, 250),
        5 => Color::Rgb(240, 130, 240),
        6 => Color::Rgb(120, 230, 230),
        7 => Color::Rgb(240, 240, 240),
        _ => Color::Reset,
    }
}

impl Perform for Terminal {
    fn print(&mut self, c: char) { self.put(c); }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.cursor_y += 1;
                if self.cursor_y >= self.height { self.scroll_up(); }
            }
            b'\r' => { self.cursor_x = 0; }
            b'\t' => {
                let next = ((self.cursor_x / 8) + 1) * 8;
                while self.cursor_x < next { self.put(' '); }
            }
            0x08 => { if self.cursor_x > 0 { self.cursor_x -= 1; } }
            0x07 => {}
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let mut nums: Vec<u32> = Vec::new();
        for p in params.iter() { nums.push(p[0] as u32); }
        let first = nums.first().copied().unwrap_or(0);
        let n = |i: usize, d: u32| nums.get(i).copied().unwrap_or(d).max(1) as usize;

        // ============ الاستعلامات (queries) ============
        match action {
            'c' => {
                // DA query
                let is_secondary = intermediates.contains(&b'>');
                if is_secondary {
                    // Secondary DA — نرد مثل xterm-256color
                    self.pending_replies.extend_from_slice(b"\x1b[>0;95;0c");
                } else {
                    // Primary DA — xterm-256color مع 256 ألوان + ANSI colors
                    self.pending_replies.extend_from_slice(b"\x1b[?62;4;6;22c");
                }
                return;
            }
            'n' => {
                match first {
                    5 => {
                        // Device Status Report — OK
                        self.pending_replies.extend_from_slice(b"\x1b[0n");
                    }
                    6 => {
                        // Cursor Position Report — 1-indexed
                        let row = self.cursor_y + 1;
                        let col = self.cursor_x + 1;
                        let reply = format!("\x1b[{};{}R", row, col);
                        self.pending_replies.extend_from_slice(reply.as_bytes());
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }

        match action {
            'A' => { self.cursor_y = self.cursor_y.saturating_sub(n(0, 1)); }
            'B' => { self.cursor_y = (self.cursor_y + n(0, 1)).min(self.height.saturating_sub(1)); }
            'C' => { self.cursor_x = (self.cursor_x + n(0, 1)).min(self.width.saturating_sub(1)); }
            'D' => { self.cursor_x = self.cursor_x.saturating_sub(n(0, 1)); }
            'E' => {
                self.cursor_y = (self.cursor_y + n(0, 1)).min(self.height.saturating_sub(1));
                self.cursor_x = 0;
            }
            'F' => {
                self.cursor_y = self.cursor_y.saturating_sub(n(0, 1));
                self.cursor_x = 0;
            }
            'G' => { self.cursor_x = (n(0, 1) - 1).min(self.width.saturating_sub(1)); }
            'H' | 'f' => {
                let row = nums.first().copied().unwrap_or(1).max(1) as usize - 1;
                let col = nums.get(1).copied().unwrap_or(1).max(1) as usize - 1;
                self.cursor_y = row.min(self.height.saturating_sub(1));
                self.cursor_x = col.min(self.width.saturating_sub(1));
            }
            'd' => {
                let row = n(0, 1) - 1;
                self.cursor_y = row.min(self.height.saturating_sub(1));
            }
            'J' => {
                match first {
                    0 => {
                        let start = self.idx(self.cursor_x, self.cursor_y);
                        for i in start..self.grid.len() { self.grid[i] = TermCell::default(); }
                    }
                    1 => {
                        let end = self.idx(self.cursor_x, self.cursor_y);
                        for i in 0..=end.min(self.grid.len().saturating_sub(1)) {
                            self.grid[i] = TermCell::default();
                        }
                    }
                    2 | 3 => { self.clear_grid(); }
                    _ => {}
                }
            }
            'K' => {
                let row_start = self.cursor_y * self.width;
                match first {
                    0 => {
                        let from = row_start + self.cursor_x;
                        let to = row_start + self.width;
                        for i in from..to.min(self.grid.len()) { self.grid[i] = TermCell::default(); }
                    }
                    1 => {
                        let from = row_start;
                        let to = row_start + self.cursor_x + 1;
                        for i in from..to.min(self.grid.len()) { self.grid[i] = TermCell::default(); }
                    }
                    2 => {
                        for i in row_start..(row_start + self.width).min(self.grid.len()) {
                            self.grid[i] = TermCell::default();
                        }
                    }
                    _ => {}
                }
            }
            'm' => { self.apply_sgr(&nums); }
            's' => { self.saved = (self.cursor_x, self.cursor_y); }
            'u' => {
                self.cursor_x = self.saved.0.min(self.width.saturating_sub(1));
                self.cursor_y = self.saved.1.min(self.height.saturating_sub(1));
            }
            'h' | 'l' => {
                if first == 25 { self.cursor_visible = action == 'h'; }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _inter: &[u8], _ignore: bool, b: u8) {
        // ESC Z = DECID (identify terminal) — بعض shells تسأله
        if b == b'Z' {
            self.pending_replies.extend_from_slice(b"\x1b[?6c");
        }
    }
    fn osc_dispatch(&mut self, _p: &[&[u8]], _b: bool) {}
    fn hook(&mut self, _p: &Params, _i: &[u8], _ig: bool, _a: char) {}
    fn put(&mut self, _b: u8) {}
    fn unhook(&mut self) {}
}

impl Terminal {
    fn apply_sgr(&mut self, nums: &[u32]) {
        if nums.is_empty() {
            self.cur_fg = Color::Reset;
            self.cur_bg = Color::Reset;
            self.cur_bold = false;
            return;
        }
        let mut i = 0usize;
        while i < nums.len() {
            let p = nums[i];
            match p {
                0 => { self.cur_fg = Color::Reset; self.cur_bg = Color::Reset; self.cur_bold = false; }
                1 => { self.cur_bold = true; }
                22 => { self.cur_bold = false; }
                30..=37 => { self.cur_fg = ansi_color((p - 30) as u8); }
                39 => { self.cur_fg = Color::Reset; }
                40..=47 => { self.cur_bg = ansi_color((p - 40) as u8); }
                49 => { self.cur_bg = Color::Reset; }
                90..=97 => { self.cur_fg = ansi_color_bright((p - 90) as u8); }
                100..=107 => { self.cur_bg = ansi_color_bright((p - 100) as u8); }
                38 => {
                    if let Some(&mode) = nums.get(i + 1) {
                        if mode == 5 {
                            if let Some(&c) = nums.get(i + 2) {
                                self.cur_fg = Color::Indexed(c as u8);
                            }
                            i += 2;
                        } else if mode == 2 {
                            let r = nums.get(i + 2).copied().unwrap_or(0) as u8;
                            let g = nums.get(i + 3).copied().unwrap_or(0) as u8;
                            let b = nums.get(i + 4).copied().unwrap_or(0) as u8;
                            self.cur_fg = Color::Rgb(r, g, b);
                            i += 4;
                        }
                    }
                }
                48 => {
                    if let Some(&mode) = nums.get(i + 1) {
                        if mode == 5 {
                            if let Some(&c) = nums.get(i + 2) {
                                self.cur_bg = Color::Indexed(c as u8);
                            }
                            i += 2;
                        } else if mode == 2 {
                            let r = nums.get(i + 2).copied().unwrap_or(0) as u8;
                            let g = nums.get(i + 3).copied().unwrap_or(0) as u8;
                            let b = nums.get(i + 4).copied().unwrap_or(0) as u8;
                            self.cur_bg = Color::Rgb(r, g, b);
                            i += 4;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}
