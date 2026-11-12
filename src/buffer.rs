use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    cursor: (usize, usize),
}

pub struct Buffer {
    pub lines: Vec<String>,
    pub path: Option<PathBuf>,
    pub cursor: (usize, usize),
    pub preferred_col: usize,
    pub modified: bool,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    pub scroll: u16,
    pub visual_anchor: Option<(usize, usize)>,
}

impl Buffer {
    pub fn empty() -> Self {
        Self {
            lines: vec![String::new()],
            path: None, cursor: (0, 0), preferred_col: 0, modified: false,
            undo_stack: Vec::new(), redo_stack: Vec::new(),
            scroll: 0, visual_anchor: None,
        }
    }

    pub fn from_file(path: PathBuf) -> std::io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let mut lines: Vec<String> = content
            .split('\n')
            .map(|s| s.trim_end_matches('\r').to_string())
            .collect();
        if lines.is_empty() { lines.push(String::new()); }
        Ok(Self {
            lines, path: Some(path), cursor: (0, 0), preferred_col: 0,
            modified: false, undo_stack: Vec::new(), redo_stack: Vec::new(),
            scroll: 0, visual_anchor: None,
        })
    }

    pub fn filename(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "[No Name]".into())
    }

    pub fn line_count(&self) -> usize { self.lines.len() }

    pub fn line(&self, i: usize) -> &str {
        self.lines.get(i).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn current_line(&self) -> &str { self.line(self.cursor.0) }

    pub fn line_chars(&self, i: usize) -> usize {
        self.lines.get(i).map(|s| s.chars().count()).unwrap_or(0)
    }

    pub fn byte_idx(&self, line: usize, col: usize) -> usize {
        let s = self.line(line);
        s.char_indices().nth(col).map(|(i, _)| i).unwrap_or(s.len())
    }

    fn snapshot(&mut self) {
        self.undo_stack.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
        if self.undo_stack.len() > 2000 { self.undo_stack.remove(0); }
        self.redo_stack.clear();
        self.modified = true;
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo_stack.pop() {
            self.redo_stack.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
            self.lines = s.lines; self.cursor = s.cursor; self.modified = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo_stack.pop() {
            self.undo_stack.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
            self.lines = s.lines; self.cursor = s.cursor; self.modified = true;
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(p) = &self.path {
            let content = self.lines.join("\n");
            fs::write(p, &content)?;
            self.modified = false;
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "no file name"))
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> std::io::Result<()> {
        let content = self.lines.join("\n");
        fs::write(&path, content)?;
        self.path = Some(path);
        self.modified = false;
        Ok(())
    }

    pub fn clamp_cursor(&mut self) {
        if self.lines.is_empty() { self.lines.push(String::new()); }
        let max_line = self.lines.len() - 1;
        if self.cursor.0 > max_line { self.cursor.0 = max_line; }
        let len = self.line_chars(self.cursor.0);
        if self.cursor.1 > len { self.cursor.1 = len; }
    }

    pub fn move_left(&mut self) {
        if self.cursor.1 > 0 { self.cursor.1 -= 1; }
        self.preferred_col = self.cursor.1;
    }
    pub fn move_right(&mut self) {
        let len = self.line_chars(self.cursor.0);
        if self.cursor.1 < len { self.cursor.1 += 1; }
        self.preferred_col = self.cursor.1;
    }
    pub fn move_up(&mut self) {
        if self.cursor.0 > 0 { self.cursor.0 -= 1; }
        let len = self.line_chars(self.cursor.0);
        self.cursor.1 = self.preferred_col.min(len);
    }
    pub fn move_down(&mut self) {
        if self.cursor.0 + 1 < self.lines.len() { self.cursor.0 += 1; }
        let len = self.line_chars(self.cursor.0);
        self.cursor.1 = self.preferred_col.min(len);
    }
    pub fn move_line_start(&mut self) { self.cursor.1 = 0; self.preferred_col = 0; }
    pub fn move_first_non_blank(&mut self) {
        let s = self.current_line();
        let idx = s.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
        self.cursor.1 = idx; self.preferred_col = idx;
    }
    pub fn move_line_end(&mut self) {
        let len = self.line_chars(self.cursor.0);
        self.cursor.1 = if len > 0 { len - 1 } else { 0 };
        self.preferred_col = self.cursor.1;
    }
    pub fn move_file_start(&mut self) { self.cursor = (0, 0); self.preferred_col = 0; }
    pub fn move_file_end(&mut self) {
        self.cursor.0 = self.lines.len().saturating_sub(1);
        self.cursor.1 = 0; self.preferred_col = 0;
    }
    pub fn goto_line(&mut self, n: usize) {
        let idx = n.saturating_sub(1);
        self.cursor.0 = idx.min(self.lines.len().saturating_sub(1));
        self.cursor.1 = 0; self.preferred_col = 0;
    }

    fn is_word_char(c: char) -> bool { c.is_alphanumeric() || c == '_' }

    pub fn move_word_forward(&mut self) {
        let mut line = self.cursor.0;
        let mut col = self.cursor.1;
        let mut chars: Vec<char> = self.line(line).chars().collect();
        while col < chars.len() && Self::is_word_char(chars[col]) { col += 1; }
        loop {
            if col >= chars.len() {
                if line + 1 < self.lines.len() {
                    line += 1; col = 0;
                    chars = self.line(line).chars().collect();
                } else { break; }
            } else if chars[col].is_whitespace() { col += 1; }
            else { break; }
        }
        self.cursor = (line, col); self.preferred_col = col;
    }

    pub fn move_word_backward(&mut self) {
        let mut line = self.cursor.0;
        let mut col = self.cursor.1;
        if col == 0 {
            if line > 0 { line -= 1; col = self.line_chars(line); }
            else { return; }
        }
        let chars: Vec<char> = self.line(line).chars().collect();
        if col > 0 { col -= 1; }
        while col > 0 && chars.get(col).map_or(false, |c| c.is_whitespace()) { col -= 1; }
        while col > 0 && chars.get(col - 1).map_or(false, |c| Self::is_word_char(*c)) { col -= 1; }
        self.cursor = (line, col); self.preferred_col = col;
    }

    pub fn move_word_end(&mut self) {
        let line = self.cursor.0;
        let chars: Vec<char> = self.line(line).chars().collect();
        if chars.is_empty() { return; }
        let mut c = self.cursor.1.min(chars.len().saturating_sub(1));
        while c + 1 < chars.len() && !chars[c + 1].is_whitespace() { c += 1; }
        self.cursor.1 = c; self.preferred_col = c;
    }

    pub fn find_char_forward(&mut self, target: char) -> bool {
        let chars: Vec<char> = self.current_line().chars().collect();
        let mut i = self.cursor.1 + 1;
        while i < chars.len() {
            if chars[i] == target { self.cursor.1 = i; return true; }
            i += 1;
        }
        false
    }

    pub fn find_char_backward(&mut self, target: char) -> bool {
        let chars: Vec<char> = self.current_line().chars().collect();
        if self.cursor.1 == 0 { return false; }
        let mut i = self.cursor.1 - 1;
        loop {
            if chars[i] == target { self.cursor.1 = i; return true; }
            if i == 0 { break; }
            i -= 1;
        }
        false
    }

    pub fn match_bracket(&mut self) {
        let chars: Vec<char> = self.current_line().chars().collect();
        if self.cursor.1 >= chars.len() { return; }
        let c = chars[self.cursor.1];
        let (open, close, forward) = match c {
            '(' => ('(', ')', true),
            ')' => ('(', ')', false),
            '[' => ('[', ']', true),
            ']' => ('[', ']', false),
            '{' => ('{', '}', true),
            '}' => ('{', '}', false),
            _ => return,
        };
        let text: Vec<char> = self.lines.join("\n").chars().collect();
        let mut base = 0;
        for i in 0..self.cursor.0 { base += self.line_chars(i) + 1; }
        let mut idx = base + self.cursor.1;
        let mut depth = 0i32;
        if forward {
            loop {
                if idx >= text.len() { return; }
                if text[idx] == c { depth += 1; }
                if text[idx] == close { depth -= 1; if depth == 0 { break; } }
                idx += 1;
            }
        } else {
            loop {
                if text[idx] == c { depth += 1; }
                if text[idx] == open { depth -= 1; if depth == 0 { break; } }
                if idx == 0 { return; }
                idx -= 1;
            }
        }
        let mut remaining = idx;
        for li in 0..self.lines.len() {
            let lc = self.line_chars(li);
            if remaining <= lc {
                self.cursor = (li, remaining);
                self.preferred_col = remaining;
                return;
            }
            remaining -= lc + 1;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.snapshot();
        let idx = self.byte_idx(self.cursor.0, self.cursor.1);
        self.lines[self.cursor.0].insert(idx, ch);
        self.cursor.1 += 1;
        self.preferred_col = self.cursor.1;
    }

    /// Insert text at the cursor as one undo step. Line breaks in `s`
    /// (`\n`, `\r\n`, `\r`) start new lines; a line never contains a newline.
    pub fn insert_str(&mut self, s: &str) {
        self.snapshot();
        self.insert_at_cursor(s);
    }

    /// Like `insert_str` but without an undo snapshot. The cursor ends just
    /// after the inserted text.
    fn insert_at_cursor(&mut self, s: &str) {
        let clean = s.replace("\r\n", "\n").replace('\r', "\n");
        let parts: Vec<&str> = clean.split('\n').collect();
        let (l, c) = self.cursor;
        let idx = self.byte_idx(l, c);
        if parts.len() == 1 {
            self.lines[l].insert_str(idx, parts[0]);
            self.cursor.1 = c + parts[0].chars().count();
        } else {
            let tail = self.lines[l].split_off(idx);
            self.lines[l].push_str(parts[0]);
            let last = parts.len() - 1;
            for (i, p) in parts[1..last].iter().enumerate() {
                self.lines.insert(l + 1 + i, p.to_string());
            }
            self.lines.insert(l + last, format!("{}{}", parts[last], tail));
            self.cursor = (l + last, parts[last].chars().count());
        }
        self.preferred_col = self.cursor.1;
    }

    /// يستبدل `n` حرفاً قبل المؤشر بـ `text` بعملية واحدة (تُستخدم للـ completion)
    pub fn replace_before_cursor(&mut self, n: usize, text: &str) {
        self.snapshot();
        let end = self.byte_idx(self.cursor.0, self.cursor.1);
        let start = self.byte_idx(self.cursor.0, self.cursor.1.saturating_sub(n));
        self.lines[self.cursor.0].replace_range(start..end, text);
        self.cursor.1 = self.cursor.1.saturating_sub(n) + text.chars().count();
        self.preferred_col = self.cursor.1;
    }

    pub fn newline(&mut self) {
        self.snapshot();
        let idx = self.byte_idx(self.cursor.0, self.cursor.1);
        let rest = self.lines[self.cursor.0].split_off(idx);
        self.lines.insert(self.cursor.0 + 1, rest);
        self.cursor = (self.cursor.0 + 1, 0);
        self.preferred_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor.1 == 0 {
            if self.cursor.0 == 0 { return; }
            self.snapshot();
            let cur = self.lines.remove(self.cursor.0);
            let prev_len = self.line_chars(self.cursor.0 - 1);
            self.lines[self.cursor.0 - 1].push_str(&cur);
            self.cursor = (self.cursor.0 - 1, prev_len);
            self.preferred_col = self.cursor.1;
        } else {
            self.snapshot();
            let idx = self.byte_idx(self.cursor.0, self.cursor.1 - 1);
            let end = self.byte_idx(self.cursor.0, self.cursor.1);
            self.lines[self.cursor.0].replace_range(idx..end, "");
            self.cursor.1 -= 1;
            self.preferred_col = self.cursor.1;
        }
    }

    pub fn delete_char_at(&mut self) {
        let len = self.line_chars(self.cursor.0);
        if self.cursor.1 >= len {
            if self.cursor.0 + 1 < self.lines.len() {
                self.snapshot();
                let next = self.lines.remove(self.cursor.0 + 1);
                self.lines[self.cursor.0].push_str(&next);
            }
            return;
        }
        self.snapshot();
        let idx = self.byte_idx(self.cursor.0, self.cursor.1);
        let end = self.byte_idx(self.cursor.0, self.cursor.1 + 1);
        self.lines[self.cursor.0].replace_range(idx..end, "");
    }

    pub fn delete_line(&mut self) -> String {
        self.snapshot();
        let removed = if self.cursor.0 < self.lines.len() {
            self.lines.remove(self.cursor.0)
        } else { String::new() };
        if self.lines.is_empty() { self.lines.push(String::new()); }
        if self.cursor.0 >= self.lines.len() { self.cursor.0 = self.lines.len() - 1; }
        self.cursor.1 = 0;
        let mut out = removed; out.push('\n'); out
    }

    pub fn yank_line(&self) -> String {
        let mut s = self.line(self.cursor.0).to_string();
        s.push('\n'); s
    }

    pub fn delete_word(&mut self) -> String {
        let start = self.cursor.1;
        self.move_word_forward();
        let end = self.cursor.1;
        let li = self.cursor.0;
        let s: Vec<char> = self.lines[li].chars().collect();
        let removed: String = s[start.min(s.len())..end.min(s.len())].iter().collect();
        let new_line: String = s[..start.min(s.len())].iter()
            .chain(s[end.min(s.len())..].iter()).collect();
        self.lines[li] = new_line;
        self.cursor.1 = start; self.preferred_col = start;
        removed
    }

    /// Vim's `p` (`after`) / `P`. Text ending in a newline is pasted as whole
    /// lines below/above the cursor line (cursor on the first pasted line);
    /// anything else is pasted as characters after/at the cursor. Multi-line
    /// text becomes real lines. One undo step.
    pub fn paste(&mut self, text: &str, after: bool) {
        let clean = text.replace("\r\n", "\n").replace('\r', "\n");
        if clean.is_empty() { return; }
        self.snapshot();
        if let Some(body) = clean.strip_suffix('\n') {
            // exactly one trailing newline marks "whole lines"; `a\n\n` is the
            // line `a` followed by an empty line
            let at = if after { self.cursor.0 + 1 } else { self.cursor.0 };
            for (i, l) in body.split('\n').enumerate() {
                self.lines.insert(at + i, l.to_string());
            }
            self.cursor.0 = at;
            let first = self.lines[at].chars().position(|c| !c.is_whitespace()).unwrap_or(0);
            self.cursor.1 = first;
        } else {
            let lc = self.line_chars(self.cursor.0);
            if after && lc > 0 { self.cursor.1 = (self.cursor.1 + 1).min(lc); }
            let start = self.cursor;
            self.insert_at_cursor(&clean);
            if clean.contains('\n') {
                self.cursor = start; // multi-line: cursor at the start of the pasted text
            } else if self.cursor.1 > 0 {
                self.cursor.1 -= 1; // single line: on the last pasted character
            }
        }
        self.preferred_col = self.cursor.1;
    }

    pub fn indent_line(&mut self) {
        self.snapshot();
        self.lines[self.cursor.0].insert_str(0, "    ");
    }

    pub fn outdent_line(&mut self) {
        self.snapshot();
        let line = self.lines[self.cursor.0].clone();
        let strip: String = line.chars().take_while(|c| *c == ' ').take(4).collect();
        let n = strip.chars().count();
        self.lines[self.cursor.0] = line.chars().skip(n).collect();
    }

    pub fn replace_all(&mut self, from: &str, to: &str) -> usize {
        self.snapshot();
        let mut count = 0;
        for line in &mut self.lines {
            let n = line.matches(from).count();
            if n > 0 { *line = line.replace(from, to); count += n; }
        }
        count
    }

    pub fn replace_line(&mut self, from: &str, to: &str) -> usize {
        self.snapshot();
        let n = self.lines[self.cursor.0].matches(from).count();
        if n > 0 { self.lines[self.cursor.0] = self.lines[self.cursor.0].replace(from, to); }
        n
    }

    pub fn find_all(&self, pattern: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        if pattern.is_empty() { return out; }
        for (li, line) in self.lines.iter().enumerate() {
            let mut start = 0usize;
            while start <= line.len() {
                if let Some(pos) = line[start..].find(pattern) {
                    let byte_pos = start + pos;
                    let char_col = line[..byte_pos].chars().count();
                    out.push((li, char_col));
                    start = byte_pos + pattern.len().max(1);
                } else { break; }
            }
        }
        out
    }

    pub fn goto_hit(&mut self, hit: (usize, usize)) {
        self.cursor = hit; self.preferred_col = hit.1;
    }

    pub fn word_under_cursor(&self) -> Option<String> {
        let chars: Vec<char> = self.current_line().chars().collect();
        if chars.is_empty() { return None; }
        let c = self.cursor.1.min(chars.len() - 1);
        if !Self::is_word_char(chars[c]) { return None; }
        let mut start = c;
        while start > 0 && Self::is_word_char(chars[start - 1]) { start -= 1; }
        let mut end = c;
        while end + 1 < chars.len() && Self::is_word_char(chars[end + 1]) { end += 1; }
        Some(chars[start..=end].iter().collect())
    }

    pub fn visual_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.visual_anchor?;
        let b = self.cursor;
        let (s, e) = if (a.0, a.1) <= (b.0, b.1) { (a, b) } else { (b, a) };
        Some((s, e))
    }

    pub fn delete_selection(&mut self) -> String {
        let (start, end) = match self.visual_range() { Some(r) => r, None => return String::new() };
        self.snapshot();
        let mut removed = String::new();
        if start.0 == end.0 {
            let s: Vec<char> = self.lines[start.0].chars().collect();
            let a = start.1.min(s.len());
            let b = (end.1 + 1).min(s.len());
            removed = s[a..b].iter().collect();
            let new_line: String = s[..a].iter().chain(s[b..].iter()).collect();
            self.lines[start.0] = new_line;
            self.cursor = (start.0, a);
        } else {
            let first: Vec<char> = self.lines[start.0].chars().collect();
            let last: Vec<char> = self.lines[end.0].chars().collect();
            let head: String = first[..start.1.min(first.len())].iter().collect();
            let tail: String = last[(end.1 + 1).min(last.len())..].iter().collect();
            for i in start.0..=end.0 {
                let l = &self.lines[i];
                removed.push_str(l);
                if i != end.0 { removed.push('\n'); }
            }
            let merged = format!("{}{}", head, tail);
            for i in (start.0 + 1..=end.0).rev() { self.lines.remove(i); }
            self.lines[start.0] = merged;
            self.cursor = (start.0, start.1);
        }
        self.preferred_col = self.cursor.1;
        self.visual_anchor = None;
        removed
    }

    pub fn yank_selection(&mut self) -> String {
        let (start, end) = match self.visual_range() { Some(r) => r, None => return String::new() };
        let mut out = String::new();
        if start.0 == end.0 {
            let s: Vec<char> = self.lines[start.0].chars().collect();
            let a = start.1.min(s.len());
            let b = (end.1 + 1).min(s.len());
            out = s[a..b].iter().collect();
        } else {
            for i in start.0..=end.0 {
                let l: Vec<char> = self.lines[i].chars().collect();
                if i == start.0 {
                    out.extend(l[start.1.min(l.len())..].iter());
                } else if i == end.0 {
                    out.push('\n');
                    out.extend(l[..(end.1 + 1).min(l.len())].iter());
                } else {
                    out.push('\n');
                    out.extend(l.iter());
                }
            }
        }
        self.visual_anchor = None;
        out
    }

    /// يجمع كل النص
    pub fn select_all(&self) -> String {
        self.lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(lines: &[&str], cursor: (usize, usize)) -> Buffer {
        let mut b = Buffer::empty();
        b.lines = lines.iter().map(|s| s.to_string()).collect();
        b.cursor = cursor;
        b
    }

    /// The bug this guards against: a newline stored *inside* a line.
    fn assert_no_embedded_newlines(b: &Buffer) {
        for (i, l) in b.lines.iter().enumerate() {
            assert!(!l.contains('\n') && !l.contains('\r'), "line {} contains a line break: {:?}", i, l);
        }
    }

    #[test]
    fn multiline_text_pasted_with_p_becomes_real_lines() {
        let mut b = buf(&["abc"], (0, 0));
        b.paste("X\nY", true);
        assert_eq!(b.lines, ["aX", "Ybc"]);
        assert_eq!(b.cursor, (0, 1)); // start of the pasted text, like Vim
        assert_no_embedded_newlines(&b);
    }

    #[test]
    fn multiline_text_pasted_with_capital_p() {
        let mut b = buf(&["abc"], (0, 1));
        b.paste("X\nY\nZ", false);
        assert_eq!(b.lines, ["aX", "Y", "Zbc"]);
        assert_eq!(b.cursor, (0, 1));
        assert_no_embedded_newlines(&b);
    }

    #[test]
    fn a_command_copied_from_a_browser_pastes_in_place() {
        // single line, no trailing newline: characters after the cursor
        let mut b = buf(&["hello"], (0, 0));
        b.paste("command", true);
        assert_eq!(b.lines, ["hcommandello"]);
        assert_eq!(b.cursor, (0, 7)); // on the last pasted character
    }

    #[test]
    fn paste_at_end_and_on_an_empty_line() {
        let mut b = buf(&["ab"], (0, 1));
        b.paste("X", true);
        assert_eq!(b.lines, ["abX"]);
        let mut b = buf(&[""], (0, 0));
        b.paste("p\nq", true);
        assert_eq!(b.lines, ["p", "q"]);
        assert_no_embedded_newlines(&b);
    }

    #[test]
    fn whole_lines_go_below_or_above_and_the_cursor_follows() {
        let mut b = buf(&["a", "b"], (0, 0));
        b.paste("x\ny\n", true);
        assert_eq!(b.lines, ["a", "x", "y", "b"]);
        assert_eq!(b.cursor, (1, 0)); // first pasted line
        let mut b = buf(&["a", "b"], (1, 0));
        b.paste("x\ny\n", false);
        assert_eq!(b.lines, ["a", "x", "y", "b"]);
        assert_eq!(b.cursor, (1, 0));
    }

    #[test]
    fn whole_lines_land_on_the_first_non_blank() {
        let mut b = buf(&["a"], (0, 0));
        b.paste("    indented\n", true);
        assert_eq!(b.lines, ["a", "    indented"]);
        assert_eq!(b.cursor, (1, 4));
    }

    #[test]
    fn a_blank_line_at_the_end_of_yanked_lines_is_kept() {
        let mut b = buf(&["z"], (0, 0));
        b.paste("a\n\n", true); // the line "a" followed by an empty line
        assert_eq!(b.lines, ["z", "a", ""]);
        let mut b = buf(&["z"], (0, 0));
        b.paste("\n", true); // a single empty line
        assert_eq!(b.lines, ["z", ""]);
    }

    #[test]
    fn windows_line_endings_are_handled() {
        let mut b = buf(&["x"], (0, 0));
        b.paste("a\r\nb\r\nc", true);
        assert_eq!(b.lines, ["xa", "b", "c"]);
        assert_no_embedded_newlines(&b);
        let mut b = buf(&["x"], (0, 0));
        b.paste("a\r\nb\r\n", true);
        assert_eq!(b.lines, ["x", "a", "b"]);
    }

    #[test]
    fn insert_str_splits_lines_too() {
        // Insert-mode Ctrl+v and bracketed paste both go through insert_str
        let mut b = buf(&["ab"], (0, 1));
        b.insert_str("X\nY");
        assert_eq!(b.lines, ["aX", "Yb"]);
        assert_eq!(b.cursor, (1, 1)); // just after the inserted text
        assert_no_embedded_newlines(&b);
        let mut b = buf(&["ab"], (0, 2));
        b.insert_str("1\n2\n3");
        assert_eq!(b.lines, ["ab1", "2", "3"]);
        assert_eq!(b.cursor, (2, 1));
    }

    #[test]
    fn one_undo_step_undoes_the_whole_paste() {
        let big: String = (0..500).map(|i| format!("line {}\n", i)).collect();
        let mut b = buf(&["start"], (0, 0));
        b.paste(&big, true);
        assert_eq!(b.lines.len(), 501);
        b.undo();
        assert_eq!(b.lines, ["start"]);
        let mut b = buf(&["start"], (0, 5));
        b.insert_str(big.trim_end());
        assert_eq!(b.lines.len(), 500);
        b.undo();
        assert_eq!(b.lines, ["start"]);
    }

    #[test]
    fn unicode_multiline_is_safe() {
        let mut b = buf(&["é"], (0, 0));
        b.paste("مرحبا\nبالعالم", true);
        assert_eq!(b.lines, ["éمرحبا", "بالعالم"]);
        assert_no_embedded_newlines(&b);
    }

    #[test]
    fn empty_paste_changes_nothing() {
        let mut b = buf(&["abc"], (0, 1));
        b.paste("", true);
        assert_eq!(b.lines, ["abc"]);
        assert!(b.undo_stack.is_empty(), "an empty paste must not create an undo step");
    }
}
