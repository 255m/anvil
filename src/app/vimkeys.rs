//! Vim-style multi-key commands (`dw`, `ci"`, `gU`, `zz`, `f,` ...) and the
//! which-key hint tables that describe them.
//!
//! `parse()` is the single source of truth: it turns the keys typed so far
//! into "need more keys", "invalid", or a finished command. The hint popup
//! is generated from the same grammar, so it never lists a key that does
//! nothing.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Mode};
use crate::buffer::Buffer;

/// (line, column) in characters.
pub type Pos = (usize, usize);

/// Pending-sequence marker for Ctrl+w.
pub const CTRL_W: &str = "\u{17}";
const CTRL_W_CH: char = '\u{17}';

// ============================================================
// Grammar
// ============================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op { Delete, Yank, Change, Indent, Outdent, Lower, Upper, Toggle }

impl Op {
    fn verb(self) -> &'static str {
        match self {
            Op::Delete => "Delete", Op::Yank => "Yank", Op::Change => "Change",
            Op::Indent => "Indent", Op::Outdent => "Outdent",
            Op::Lower => "Lowercase", Op::Upper => "Uppercase", Op::Toggle => "Toggle case",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motion {
    WordFwd, WordBack, WordEnd, LineStart, FirstNonBlank, LineEnd,
    Left, Right, Down, Up, FileStart, FileEnd, ParaFwd, ParaBack, Bracket,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// Doubled operator key: `dd`, `yy`, `>>`, `guu` ...
    Line,
    Motion(Motion),
    /// `f` `t` `F` `T` + char
    Find(char, char),
    /// `i`/`a` + object char (inner?, canonical char)
    Object(bool, char),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parse {
    /// Valid prefix, waiting for more keys.
    Need,
    /// Not a valid sequence: cancel.
    Bad,
    Op(Op, Target),
    Find(char, char),
    Replace(char),
    SetMark(char),
    /// (exact position?, mark)
    JumpMark(bool, char),
    FileStart,
    PrevWordEnd,
    LastInsert,
    Scroll(char),
    SaveQuit,
    QuitNoSave,
    Pane(char),
}

/// Split a sequence into (operator, key that doubles it, remaining keys).
fn split_op(c: &[char]) -> Option<(Op, char, &[char])> {
    match c {
        ['d', rest @ ..] => Some((Op::Delete, 'd', rest)),
        ['y', rest @ ..] => Some((Op::Yank, 'y', rest)),
        ['c', rest @ ..] => Some((Op::Change, 'c', rest)),
        ['>', rest @ ..] => Some((Op::Indent, '>', rest)),
        ['<', rest @ ..] => Some((Op::Outdent, '<', rest)),
        ['g', 'u', rest @ ..] => Some((Op::Lower, 'u', rest)),
        ['g', 'U', rest @ ..] => Some((Op::Upper, 'U', rest)),
        ['g', '~', rest @ ..] => Some((Op::Toggle, '~', rest)),
        _ => None,
    }
}

fn motion_for(k: char) -> Option<Motion> {
    Some(match k {
        'w' => Motion::WordFwd, 'b' => Motion::WordBack, 'e' => Motion::WordEnd,
        '0' => Motion::LineStart, '^' => Motion::FirstNonBlank, '$' => Motion::LineEnd,
        'h' => Motion::Left, 'l' => Motion::Right, 'j' => Motion::Down, 'k' => Motion::Up,
        'G' => Motion::FileEnd, '}' => Motion::ParaFwd, '{' => Motion::ParaBack,
        '%' => Motion::Bracket,
        _ => return None,
    })
}

/// Canonical text-object character.
fn obj_char(o: char) -> Option<char> {
    match o {
        'w' | 'p' | '"' | '\'' | '`' => Some(o),
        '(' | ')' | 'b' => Some('('),
        '[' | ']' => Some('['),
        '{' | '}' | 'B' => Some('{'),
        '<' | '>' => Some('<'),
        _ => None,
    }
}

fn parse_target(op: Op, dbl: char, rest: &[char]) -> Parse {
    match rest {
        [] => Parse::Need,
        [k] if *k == dbl => Parse::Op(op, Target::Line),
        [k] if motion_for(*k).is_some() => {
            Parse::Op(op, Target::Motion(motion_for(*k).unwrap()))
        }
        ['g'] => Parse::Need,
        ['g', 'g'] => Parse::Op(op, Target::Motion(Motion::FileStart)),
        ['i'] | ['a'] | ['f'] | ['t'] | ['F'] | ['T'] => Parse::Need,
        [k @ ('i' | 'a'), o] => match obj_char(*o) {
            Some(c) => Parse::Op(op, Target::Object(*k == 'i', c)),
            None => Parse::Bad,
        },
        [k @ ('f' | 't' | 'F' | 'T'), ch] => Parse::Op(op, Target::Find(*k, *ch)),
        _ => Parse::Bad,
    }
}

pub fn parse(seq: &str) -> Parse {
    let c: Vec<char> = seq.chars().collect();
    if let Some((op, dbl, rest)) = split_op(&c) {
        return parse_target(op, dbl, rest);
    }
    match c.as_slice() {
        ['g'] | ['z'] | ['Z'] | ['r'] | ['m'] | ['\''] | ['`']
        | ['f'] | ['t'] | ['F'] | ['T'] | [CTRL_W_CH] => Parse::Need,
        ['g', 'g'] => Parse::FileStart,
        ['g', 'e'] => Parse::PrevWordEnd,
        ['g', 'i'] => Parse::LastInsert,
        ['z', k @ ('z' | 't' | 'b')] => Parse::Scroll(*k),
        ['Z', 'Z'] => Parse::SaveQuit,
        ['Z', 'Q'] => Parse::QuitNoSave,
        ['r', ch] => Parse::Replace(*ch),
        ['m', ch] if ch.is_ascii_lowercase() => Parse::SetMark(*ch),
        ['\'', ch] if ch.is_ascii_lowercase() => Parse::JumpMark(false, *ch),
        ['`', ch] if ch.is_ascii_lowercase() => Parse::JumpMark(true, *ch),
        [k @ ('f' | 't' | 'F' | 'T'), ch] => Parse::Find(*k, *ch),
        [CTRL_W_CH, k @ ('h' | 'j' | 'k' | 'l')] => Parse::Pane(*k),
        _ => Parse::Bad,
    }
}

// ============================================================
// Hints (generated from the same grammar)
// ============================================================

pub struct Hint {
    pub title: String,
    /// (key label, description). A description starting with `+` is a group
    /// that needs more keys.
    pub rows: Vec<(String, String)>,
}

fn r(k: &str, d: &str) -> (String, String) { (k.to_string(), d.to_string()) }

fn object_rows(inner: bool) -> Vec<(String, String)> {
    let w = if inner { "inner" } else { "around" };
    vec![
        r("w", &format!("{} word", w)),
        r("p", &format!("{} paragraph", w)),
        r("\"", &format!("{} \"double quotes\"", w)),
        r("'", &format!("{} 'single quotes'", w)),
        r("`", &format!("{} `backticks`", w)),
        r("( b", &format!("{} ( parentheses )", w)),
        r("[", &format!("{} [ brackets ]", w)),
        r("{ B", &format!("{} {{ braces }}", w)),
        r("<", &format!("{} < angle brackets >", w)),
    ]
}

fn target_rows(dbl: char, rest: &[char]) -> Vec<(String, String)> {
    match rest {
        [] => vec![
            r(&dbl.to_string(), "whole line"),
            r("w", "to next word"),
            r("b", "back to word start"),
            r("e", "to end of word"),
            r("0", "to line start"),
            r("^", "to first non-blank"),
            r("$", "to line end"),
            r("h", "one char left"),
            r("l", "one char right"),
            r("j", "this + next line"),
            r("k", "this + previous line"),
            r("gg", "to first line"),
            r("G", "to last line"),
            r("{", "back to blank line"),
            r("}", "to next blank line"),
            r("%", "to matching bracket"),
            r("i", "+inner text object"),
            r("a", "+around text object"),
            r("f", "+up to and incl. char"),
            r("t", "+up to before char"),
            r("F", "+back to char"),
            r("T", "+back to after char"),
        ],
        ['g'] => vec![r("g", "to first line")],
        ['i'] => object_rows(true),
        ['a'] => object_rows(false),
        ['f'] => vec![r("‹char›", "up to and including next ‹char›")],
        ['t'] => vec![r("‹char›", "up to just before next ‹char›")],
        ['F'] => vec![r("‹char›", "back to previous ‹char›")],
        ['T'] => vec![r("‹char›", "back to just after previous ‹char›")],
        _ => vec![],
    }
}

/// Hint for the keys typed so far, or `None` if the sequence is not pending.
pub fn hint(seq: &str, count: &str) -> Option<Hint> {
    if parse(seq) != Parse::Need { return None; }
    let c: Vec<char> = seq.chars().collect();
    let shown = seq.replace(CTRL_W, "Ctrl-w");

    if let Some((op, dbl, rest)) = split_op(&c) {
        return Some(Hint {
            title: format!(" {}{} \u{b7} {} ", count, shown, op.verb()),
            rows: target_rows(dbl, rest),
        });
    }

    let rows = match c.as_slice() {
        ['g'] => vec![
            r("g", "first line (or line [count])"),
            r("e", "previous end of word"),
            r("i", "insert where you last left insert"),
            r("u", "+lowercase (then a motion)"),
            r("U", "+uppercase (then a motion)"),
            r("~", "+toggle case (then a motion)"),
        ],
        ['z'] => vec![
            r("z", "center cursor line"),
            r("t", "cursor line to top"),
            r("b", "cursor line to bottom"),
        ],
        ['Z'] => vec![r("Z", "save and close tab"), r("Q", "close tab, discard changes")],
        ['r'] => vec![r("‹char›", "replace character under cursor")],
        ['m'] => vec![r("a-z", "set mark")],
        ['\''] => vec![r("a-z", "jump to mark (line)")],
        ['`'] => vec![r("a-z", "jump to mark (exact position)")],
        ['f'] => vec![r("‹char›", "jump to next ‹char› on the line")],
        ['t'] => vec![r("‹char›", "jump to just before next ‹char›")],
        ['F'] => vec![r("‹char›", "jump back to previous ‹char›")],
        ['T'] => vec![r("‹char›", "jump back to just after previous ‹char›")],
        [CTRL_W_CH] => vec![
            r("h", "focus pane left"),
            r("j", "focus pane below"),
            r("k", "focus pane above"),
            r("l", "focus pane right"),
        ],
        _ => return None,
    };
    Some(Hint { title: format!(" {}{} ", count, shown), rows })
}

// ============================================================
// Motions & text objects (pure functions over a Buffer)
// ============================================================

/// A resolved edit region. Charwise: `[start, end)`, may span lines.
/// Linewise: every line `start.0 ..= end.0`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range { pub start: Pos, pub end: Pos, pub linewise: bool }

fn cr(a: Pos, b: Pos) -> Range {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    Range { start, end, linewise: false }
}

fn chars_of(buf: &Buffer, l: usize) -> Vec<char> { buf.line(l).chars().collect() }

/// 0 = whitespace, 1 = word character, 2 = punctuation.
fn class(c: char) -> u8 {
    if c.is_whitespace() { 0 } else if c.is_alphanumeric() || c == '_' { 1 } else { 2 }
}

fn first_non_blank(buf: &Buffer, l: usize) -> usize {
    let chars = chars_of(buf, l);
    chars.iter().position(|c| !c.is_whitespace()).unwrap_or(chars.len())
}

/// Vim `w`: start of the next word (an empty line counts as a word).
fn word_fwd(buf: &Buffer, pos: Pos) -> Pos {
    let (mut l, mut c) = pos;
    let last = buf.line_count().saturating_sub(1);
    let mut chars = chars_of(buf, l);
    if c < chars.len() {
        let cls = class(chars[c]);
        if cls != 0 { while c < chars.len() && class(chars[c]) == cls { c += 1; } }
    }
    loop {
        while c < chars.len() && class(chars[c]) == 0 { c += 1; }
        if c < chars.len() { return (l, c); }
        if l >= last { return (l, chars.len()); }
        l += 1; c = 0; chars = chars_of(buf, l);
        if chars.is_empty() { return (l, 0); }
    }
}

/// Vim `b`: start of the previous word.
fn word_back(buf: &Buffer, pos: Pos) -> Pos {
    let (mut l, mut c) = pos;
    let mut chars = chars_of(buf, l);
    c = c.min(chars.len());
    loop {
        if c == 0 {
            if l == 0 { return (0, 0); }
            l -= 1; chars = chars_of(buf, l); c = chars.len();
            if chars.is_empty() { return (l, 0); }
            continue;
        }
        c -= 1;
        if class(chars[c]) == 0 { continue; }
        break;
    }
    let cls = class(chars[c]);
    while c > 0 && class(chars[c - 1]) == cls { c -= 1; }
    (l, c)
}

/// Vim `e`: last character of the next word.
fn word_end(buf: &Buffer, pos: Pos) -> Pos {
    let (mut l, mut c) = pos;
    let last = buf.line_count().saturating_sub(1);
    let mut chars = chars_of(buf, l);
    c += 1;
    loop {
        if c >= chars.len() {
            if l >= last { return (l, chars.len().saturating_sub(1)); }
            l += 1; c = 0; chars = chars_of(buf, l);
            continue;
        }
        if class(chars[c]) == 0 { c += 1; continue; }
        break;
    }
    let cls = class(chars[c]);
    while c + 1 < chars.len() && class(chars[c + 1]) == cls { c += 1; }
    (l, c)
}

/// Vim `ge`: last character of the previous word.
pub fn word_end_back(buf: &Buffer, pos: Pos) -> Pos {
    let (mut l, mut c) = pos;
    let mut chars = chars_of(buf, l);
    c = c.min(chars.len());
    if c < chars.len() && class(chars[c]) != 0 {
        let cls = class(chars[c]);
        while c > 0 && class(chars[c - 1]) == cls { c -= 1; }
    }
    loop {
        if c == 0 {
            if l == 0 { return (0, 0); }
            l -= 1; chars = chars_of(buf, l); c = chars.len();
            if chars.is_empty() { return (l, 0); }
            continue;
        }
        c -= 1;
        if class(chars[c]) == 0 { continue; }
        return (l, c);
    }
}

fn grid(buf: &Buffer) -> Vec<Vec<char>> {
    (0..buf.line_count()).map(|l| chars_of(buf, l)).collect()
}

/// Next character position, skipping line breaks and empty lines.
fn step_fwd(g: &[Vec<char>], (l, c): Pos) -> Option<Pos> {
    if c + 1 < g[l].len() { return Some((l, c + 1)); }
    let mut nl = l + 1;
    while nl < g.len() && g[nl].is_empty() { nl += 1; }
    if nl < g.len() { Some((nl, 0)) } else { None }
}

fn step_back(g: &[Vec<char>], (l, c): Pos) -> Option<Pos> {
    if c > 0 { return Some((l, c - 1)); }
    let mut pl = l;
    loop {
        if pl == 0 { return None; }
        pl -= 1;
        if !g[pl].is_empty() { return Some((pl, g[pl].len() - 1)); }
    }
}

/// Innermost `open ... close` pair around `cur` (searches across lines).
fn enclosing(g: &[Vec<char>], cur: Pos, open: char, close: char) -> Option<(Pos, Pos)> {
    let mut depth = 0usize;
    let mut p = cur;
    let opener;
    loop {
        let ch = g.get(p.0).and_then(|row| row.get(p.1)).copied();
        if let Some(ch) = ch {
            if p == cur {
                if ch == open { opener = p; break; }
                // a closing bracket under the cursor belongs to its own pair
            } else if ch == close {
                depth += 1;
            } else if ch == open {
                if depth == 0 { opener = p; break; }
                depth -= 1;
            }
        }
        p = step_back(g, p)?;
    }
    let mut depth = 0usize;
    let mut q = opener;
    loop {
        q = step_fwd(g, q)?;
        let ch = g[q.0][q.1];
        if ch == open { depth += 1; }
        else if ch == close {
            if depth == 0 { return Some((opener, q)); }
            depth -= 1;
        }
    }
}

/// `%`: partner of the bracket under the cursor.
fn bracket_partner(g: &[Vec<char>], cur: Pos) -> Option<Pos> {
    let ch = *g.get(cur.0)?.get(cur.1)?;
    let (open, close, fwd) = match ch {
        '(' => ('(', ')', true), ')' => ('(', ')', false),
        '[' => ('[', ']', true), ']' => ('[', ']', false),
        '{' => ('{', '}', true), '}' => ('{', '}', false),
        _ => return None,
    };
    let mut depth = 0usize;
    let mut p = cur;
    loop {
        p = if fwd { step_fwd(g, p)? } else { step_back(g, p)? };
        let c = g[p.0][p.1];
        if fwd {
            if c == open { depth += 1; }
            else if c == close { if depth == 0 { return Some(p); } depth -= 1; }
        } else if c == close { depth += 1; }
        else if c == open { if depth == 0 { return Some(p); } depth -= 1; }
    }
}

fn word_obj(buf: &Buffer, inner: bool, cur: Pos) -> Option<Range> {
    let chars = chars_of(buf, cur.0);
    if chars.is_empty() { return None; }
    let c = cur.1.min(chars.len() - 1);
    let cls = class(chars[c]);
    let mut s = c;
    while s > 0 && class(chars[s - 1]) == cls { s -= 1; }
    let mut e = c + 1;
    while e < chars.len() && class(chars[e]) == cls { e += 1; }
    if !inner {
        if cls != 0 {
            let mut e2 = e;
            while e2 < chars.len() && class(chars[e2]) == 0 { e2 += 1; }
            if e2 > e { e = e2; }
            else { while s > 0 && class(chars[s - 1]) == 0 { s -= 1; } }
        } else if e < chars.len() {
            let c2 = class(chars[e]);
            while e < chars.len() && class(chars[e]) == c2 { e += 1; }
        }
    }
    Some(cr((cur.0, s), (cur.0, e)))
}

fn quote_obj(buf: &Buffer, inner: bool, cur: Pos, q: char) -> Option<Range> {
    let chars = chars_of(buf, cur.0);
    let mut idx: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' { i += 2; continue; }
        if chars[i] == q { idx.push(i); }
        i += 1;
    }
    let c = cur.1;
    let mut pair = idx.chunks(2).find(|w| w.len() == 2 && w[0] <= c && c <= w[1]);
    if pair.is_none() { pair = idx.chunks(2).find(|w| w.len() == 2 && w[0] > c); }
    let (s, e) = pair.map(|w| (w[0], w[1]))?;
    let l = cur.0;
    if inner { return Some(cr((l, s + 1), (l, e))); }
    let mut e2 = e + 1;
    while e2 < chars.len() && class(chars[e2]) == 0 { e2 += 1; }
    if e2 > e + 1 { return Some(cr((l, s), (l, e2))); }
    let mut s2 = s;
    while s2 > 0 && class(chars[s2 - 1]) == 0 { s2 -= 1; }
    Some(cr((l, s2), (l, e + 1)))
}

fn bracket_obj(buf: &Buffer, inner: bool, cur: Pos, o: char) -> Option<Range> {
    let (open, close) = match o {
        '(' => ('(', ')'), '[' => ('[', ']'), '{' => ('{', '}'), _ => ('<', '>'),
    };
    let g = grid(buf);
    let (op, cl) = enclosing(&g, cur, open, close)?;
    if !inner { return Some(cr(op, (cl.0, cl.1 + 1))); }
    // A block whose brackets sit on their own lines: act on the lines between.
    let opener_last = op.1 + 1 >= g[op.0].len();
    let closer_alone = g[cl.0][..cl.1].iter().all(|c| c.is_whitespace());
    if opener_last && closer_alone && cl.0 > op.0 {
        if cl.0 - op.0 >= 2 {
            return Some(Range { start: (op.0 + 1, 0), end: (cl.0 - 1, 0), linewise: true });
        }
        return Some(cr((op.0, op.1 + 1), (op.0, op.1 + 1)));
    }
    Some(cr((op.0, op.1 + 1), cl))
}

fn para_obj(buf: &Buffer, inner: bool, cur: Pos) -> Option<Range> {
    let last = buf.line_count().saturating_sub(1);
    let blank = |l: usize| buf.line(l).trim().is_empty();
    let b = blank(cur.0);
    let (mut s, mut e) = (cur.0, cur.0);
    while s > 0 && blank(s - 1) == b { s -= 1; }
    while e < last && blank(e + 1) == b { e += 1; }
    if !inner {
        if !b {
            let mut e2 = e;
            while e2 < last && blank(e2 + 1) { e2 += 1; }
            if e2 > e { e = e2; } else { while s > 0 && blank(s - 1) { s -= 1; } }
        } else {
            while e < last && !blank(e + 1) { e += 1; }
        }
    }
    Some(Range { start: (s, 0), end: (e, 0), linewise: true })
}

/// Turn an operator target into a concrete region. `count0` is 0 when no
/// count was typed.
pub fn resolve(buf: &Buffer, op: Op, target: Target, count0: usize) -> Option<Range> {
    let n = count0.max(1);
    let cur = buf.cursor;
    let last = buf.line_count().saturating_sub(1);
    let len = |l: usize| buf.line_chars(l);
    let linew = |a: usize, b: usize| {
        Some(Range { start: (a.min(b), 0), end: (a.max(b), 0), linewise: true })
    };
    let chw = |a: Pos, b: Pos| if a == b { None } else { Some(cr(a, b)) };
    let blank = |l: usize| buf.line(l).trim().is_empty();

    match target {
        Target::Line => linew(cur.0, (cur.0 + n - 1).min(last)),
        Target::Find(k, ch) => {
            let chars = chars_of(buf, cur.0);
            if k == 'f' || k == 't' {
                let mut seen = 0;
                let mut found = None;
                for i in (cur.1 + 1)..chars.len() {
                    if chars[i] == ch { seen += 1; if seen == n { found = Some(i); break; } }
                }
                let i = found?;
                if k == 'f' { chw(cur, (cur.0, i + 1)) } else { chw(cur, (cur.0, i)) }
            } else {
                let mut seen = 0;
                let mut found = None;
                let mut i = cur.1.min(chars.len());
                while i > 0 {
                    i -= 1;
                    if chars[i] == ch { seen += 1; if seen == n { found = Some(i); break; } }
                }
                let i = found?;
                if k == 'F' { chw((cur.0, i), cur) } else { chw((cur.0, i + 1), cur) }
            }
        }
        Target::Object(inner, o) => match o {
            'w' => word_obj(buf, inner, cur),
            'p' => para_obj(buf, inner, cur),
            '"' | '\'' | '`' => quote_obj(buf, inner, cur, o),
            '(' | '[' | '{' | '<' => bracket_obj(buf, inner, cur, o),
            _ => None,
        },
        Target::Motion(m) => match m {
            Motion::WordFwd => {
                if op == Op::Change {
                    // `cw` behaves like `ce`: keep the whitespace after the word
                    let chars = chars_of(buf, cur.0);
                    if cur.1 < chars.len() && class(chars[cur.1]) != 0 {
                        let cls = class(chars[cur.1]);
                        let mut e = cur.1;
                        while e + 1 < chars.len() && class(chars[e + 1]) == cls { e += 1; }
                        let mut endp = (cur.0, e);
                        for _ in 1..n { endp = word_end(buf, endp); }
                        return chw(cur, (endp.0, endp.1 + 1));
                    }
                }
                let mut d = cur;
                for _ in 0..n { d = word_fwd(buf, d); }
                if d.0 > cur.0 {
                    // stop at the end of the line instead of joining the next one
                    let l = d.0 - 1;
                    d = (l, len(l));
                }
                chw(cur, d)
            }
            Motion::WordBack => {
                let mut d = cur;
                for _ in 0..n { d = word_back(buf, d); }
                chw(d, cur)
            }
            Motion::WordEnd => {
                let mut d = cur;
                for _ in 0..n { d = word_end(buf, d); }
                chw(cur, (d.0, d.1 + 1))
            }
            Motion::LineStart => chw((cur.0, 0), cur),
            Motion::FirstNonBlank => chw((cur.0, first_non_blank(buf, cur.0)), cur),
            Motion::LineEnd => {
                let el = (cur.0 + n - 1).min(last);
                chw(cur, (el, len(el)))
            }
            Motion::Left => chw((cur.0, cur.1.saturating_sub(n)), cur),
            Motion::Right => chw(cur, (cur.0, (cur.1 + n).min(len(cur.0)))),
            Motion::Down => linew(cur.0, (cur.0 + n).min(last)),
            Motion::Up => linew(cur.0.saturating_sub(n), cur.0),
            Motion::FileEnd => {
                let t = if count0 > 0 { (count0 - 1).min(last) } else { last };
                linew(cur.0, t)
            }
            Motion::FileStart => {
                let t = if count0 > 0 { (count0 - 1).min(last) } else { 0 };
                linew(cur.0, t)
            }
            Motion::ParaFwd => {
                let mut i = cur.0;
                for _ in 0..n {
                    while i < last && blank(i) { i += 1; }
                    while i < last && !blank(i) { i += 1; }
                }
                let dest = if i == last && !blank(last) { (last, len(last)) } else { (i, 0) };
                if dest.0 > cur.0 && dest.1 == 0 {
                    // exclusive motion ending in column 0: stop at the end of the
                    // previous line, or take whole lines if we started at the indent
                    if cur.1 <= first_non_blank(buf, cur.0) {
                        return linew(cur.0, dest.0 - 1);
                    }
                    let l = dest.0 - 1;
                    return chw(cur, (l, len(l)));
                }
                chw(cur, dest)
            }
            Motion::ParaBack => {
                let mut i = cur.0;
                for _ in 0..n {
                    while i > 0 && blank(i) { i -= 1; }
                    while i > 0 && !blank(i) { i -= 1; }
                }
                chw((i, 0), cur)
            }
            Motion::Bracket => {
                let g = grid(buf);
                let p = bracket_partner(&g, cur)?;
                let (a, b) = if cur <= p { (cur, p) } else { (p, cur) };
                chw(a, (b.0, b.1 + 1))
            }
        },
    }
}

// ============================================================
// Applying operators
// ============================================================

pub struct Outcome {
    /// New clipboard contents (yank / delete / change).
    pub text: Option<String>,
    /// Enter Insert mode afterwards.
    pub insert: bool,
}

fn get_text(buf: &Buffer, r: Range) -> String {
    let last = buf.line_count().saturating_sub(1);
    if r.linewise {
        let a = r.start.0.min(last);
        let b = r.end.0.min(last);
        let mut s = buf.lines[a..=b].join("\n");
        s.push('\n');
        return s;
    }
    let (s, e) = (r.start, r.end);
    if s.0 == e.0 {
        return buf.line(s.0).chars().skip(s.1).take(e.1.saturating_sub(s.1)).collect();
    }
    let mut out: String = buf.line(s.0).chars().skip(s.1).collect();
    for l in (s.0 + 1)..e.0 { out.push('\n'); out.push_str(buf.line(l)); }
    out.push('\n');
    out.extend(buf.line(e.0).chars().take(e.1));
    out
}

fn delete_range(buf: &mut Buffer, r: Range) {
    buf.snapshot();
    if r.linewise {
        let last = buf.lines.len() - 1;
        let (a, b) = (r.start.0.min(last), r.end.0.min(last));
        buf.lines.drain(a..=b);
        if buf.lines.is_empty() { buf.lines.push(String::new()); }
        buf.cursor = (a.min(buf.lines.len() - 1), 0);
    } else {
        let (s, e) = (r.start, r.end);
        let head: String = buf.lines[s.0].chars().take(s.1).collect();
        let tail: String = buf.lines[e.0].chars().skip(e.1).collect();
        buf.lines[s.0] = head + &tail;
        if e.0 > s.0 { buf.lines.drain(s.0 + 1..=e.0); }
        buf.cursor = s;
    }
    buf.preferred_col = buf.cursor.1;
}

fn map_case(op: Op, s: &str) -> String {
    s.chars().map(|c| match op {
        Op::Lower => c.to_lowercase().collect::<String>(),
        Op::Upper => c.to_uppercase().collect::<String>(),
        _ => {
            if c.is_lowercase() { c.to_uppercase().collect() }
            else if c.is_uppercase() { c.to_lowercase().collect() }
            else { c.to_string() }
        }
    }).collect()
}

pub fn apply(buf: &mut Buffer, op: Op, r: Range) -> Outcome {
    let last = buf.lines.len().saturating_sub(1);
    match op {
        Op::Yank => {
            let t = get_text(buf, r);
            if r.linewise {
                if r.start.0 < buf.cursor.0 { buf.cursor.0 = r.start.0; }
            } else {
                buf.cursor = r.start;
            }
            Outcome { text: Some(t), insert: false }
        }
        Op::Delete => {
            let t = get_text(buf, r);
            delete_range(buf, r);
            Outcome { text: Some(t), insert: false }
        }
        Op::Change => {
            let t = get_text(buf, r);
            if r.linewise {
                buf.snapshot();
                let (a, b) = (r.start.0.min(last), r.end.0.min(last));
                buf.lines.drain(a..=b);
                buf.lines.insert(a, String::new());
                buf.cursor = (a, 0);
                buf.preferred_col = 0;
            } else {
                delete_range(buf, r);
            }
            Outcome { text: Some(t), insert: true }
        }
        Op::Indent | Op::Outdent => {
            buf.snapshot();
            let (a, b) = (r.start.0.min(last), r.end.0.min(last));
            for l in a..=b {
                if op == Op::Indent {
                    if !buf.lines[l].is_empty() { buf.lines[l].insert_str(0, "    "); }
                } else if buf.lines[l].starts_with('\t') {
                    buf.lines[l].remove(0);
                } else {
                    let n = buf.lines[l].chars().take_while(|c| *c == ' ').take(4).count();
                    buf.lines[l] = buf.lines[l].chars().skip(n).collect();
                }
            }
            buf.cursor.0 = a;
            let fnb = first_non_blank(buf, a);
            buf.cursor.1 = fnb;
            buf.preferred_col = fnb;
            Outcome { text: None, insert: false }
        }
        Op::Lower | Op::Upper | Op::Toggle => {
            buf.snapshot();
            let (a, b) = (r.start.0.min(last), r.end.0.min(last));
            for l in a..=b {
                let chars: Vec<char> = buf.lines[l].chars().collect();
                let from = if r.linewise || l > r.start.0 { 0 } else { r.start.1 };
                let to = if r.linewise || l < r.end.0 { chars.len() } else { r.end.1.min(chars.len()) };
                if from >= to { continue; }
                let head: String = chars[..from].iter().collect();
                let mid: String = chars[from..to].iter().collect();
                let tail: String = chars[to..].iter().collect();
                buf.lines[l] = head + &map_case(op, &mid) + &tail;
            }
            if r.linewise { buf.cursor.0 = a; } else { buf.cursor = r.start; }
            buf.preferred_col = buf.cursor.1;
            Outcome { text: None, insert: false }
        }
    }
}

// ============================================================
// App glue
// ============================================================

impl App {
    /// Hint for whatever key sequence is currently pending.
    pub fn pending_hint(&self) -> Option<Hint> {
        if self.pending_seq.is_empty() { return None; }
        let count = if self.seq_count > 0 { self.seq_count.to_string() } else { String::new() };
        hint(&self.pending_seq, &count)
    }

    /// Start a multi-key sequence with `first`, keeping any typed count.
    pub fn begin_seq(&mut self, first: char) {
        self.seq_count = self.take_count();
        self.seq_count2.clear();
        self.pending_seq.clear();
        self.pending_seq.push(first);
    }

    pub fn clear_pending(&mut self) {
        self.pending_seq.clear();
        self.seq_count = 0;
        self.seq_count2.clear();
    }

    /// Feed one key to the pending sequence.
    pub fn handle_pending_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let in_ctrl_w = self.pending_seq == CTRL_W;
        let ch = match key.code {
            KeyCode::Char(c) if !ctrl || in_ctrl_w => c,
            KeyCode::Left if in_ctrl_w => 'h',
            KeyCode::Down if in_ctrl_w => 'j',
            KeyCode::Up if in_ctrl_w => 'k',
            KeyCode::Right if in_ctrl_w => 'l',
            _ => { self.clear_pending(); return; } // Esc, arrows, Ctrl-x ... cancel
        };

        // digits after an operator are a count: d3w, y2j ...
        let after_op = {
            let c: Vec<char> = self.pending_seq.chars().collect();
            split_op(&c).map_or(false, |(_, _, rest)| rest.is_empty())
        };
        if after_op && ch.is_ascii_digit() && (ch != '0' || !self.seq_count2.is_empty()) {
            self.seq_count2.push(ch);
            return;
        }

        self.pending_seq.push(ch);
        match parse(&self.pending_seq) {
            Parse::Need => {}
            Parse::Bad => self.clear_pending(),
            done => {
                let c1 = self.seq_count;
                let c2 = self.seq_count2.parse::<usize>().unwrap_or(0);
                let count = if c1 == 0 && c2 == 0 { 0 } else { c1.max(1) * c2.max(1) };
                self.clear_pending();
                self.run_parsed(done, count);
            }
        }
        self.buf().clamp_cursor();
        self.ensure_visible();
    }

    fn run_parsed(&mut self, p: Parse, count0: usize) {
        let n = count0.max(1);
        match p {
            Parse::Op(op, target) => {
                if let Some(range) = resolve(self.buf_ref(), op, target, count0) {
                    let out = apply(self.buf(), op, range);
                    if let Some(t) = out.text { self.clipboard = t; }
                    if out.insert {
                        self.mode = Mode::Insert;
                        self.message.clear();
                    } else {
                        // Normal mode keeps the cursor on a character
                        let l = self.buf_ref().cursor.0;
                        let len = self.buf_ref().line_chars(l);
                        if len > 0 && self.buf_ref().cursor.1 >= len {
                            self.buf().cursor.1 = len - 1;
                        }
                    }
                }
            }
            Parse::Find(k, ch) => {
                let saved = self.buf_ref().cursor;
                let fwd = k == 'f' || k == 't';
                let mut ok = true;
                for _ in 0..n {
                    let hit = if fwd { self.buf().find_char_forward(ch) }
                              else { self.buf().find_char_backward(ch) };
                    if !hit { ok = false; break; }
                }
                if !ok { self.buf().cursor = saved; return; }
                let col = self.buf_ref().cursor.1;
                if k == 't' && col > saved.1 { self.buf().cursor.1 = col - 1; }
                if k == 'T' && col + 1 < saved.1 { self.buf().cursor.1 = col + 1; }
                self.buf().preferred_col = self.buf_ref().cursor.1;
                self.last_find = Some((ch, fwd));
            }
            Parse::Replace(ch) => {
                let (l, c) = self.buf_ref().cursor;
                let mut chars: Vec<char> = self.buf_ref().line(l).chars().collect();
                if c + n <= chars.len() {
                    self.buf().snapshot();
                    for i in c..c + n { chars[i] = ch; }
                    self.buf().lines[l] = chars.into_iter().collect();
                    self.buf().cursor.1 = c + n - 1;
                }
            }
            Parse::SetMark(ch) => {
                let pos = self.buf_ref().cursor;
                self.buf().marks.insert(ch, pos);
                self.message = format!("mark '{}' set", ch);
            }
            Parse::JumpMark(exact, ch) => {
                match self.buf_ref().marks.get(&ch).copied() {
                    Some((l, c)) => {
                        let l = l.min(self.buf_ref().line_count().saturating_sub(1));
                        let col = if exact { c.min(self.buf_ref().line_chars(l)) }
                                  else { first_non_blank(self.buf_ref(), l) };
                        self.buf().cursor = (l, col);
                        self.buf().preferred_col = col;
                    }
                    None => self.message = format!("mark '{}' not set", ch),
                }
            }
            Parse::FileStart => {
                if count0 > 0 { self.buf().goto_line(count0); } else { self.buf().move_file_start(); }
            }
            Parse::PrevWordEnd => {
                let mut p = self.buf_ref().cursor;
                for _ in 0..n { p = word_end_back(self.buf_ref(), p); }
                self.buf().cursor = p;
                self.buf().preferred_col = p.1;
            }
            Parse::LastInsert => match self.buf_ref().last_insert {
                Some(p) => {
                    self.buf().cursor = p;
                    self.buf().clamp_cursor();
                    self.mode = Mode::Insert;
                    self.message.clear();
                }
                None => self.message = "no previous insert".into(),
            },
            Parse::Scroll(k) => {
                let view_h = (self.editor_view_h as usize).max(1);
                let total = self.buf_ref().line_count();
                let row = self.buf_ref().cursor.0;
                let s = match k {
                    't' => row,
                    'b' => (row + 1).saturating_sub(view_h),
                    _ => row.saturating_sub(view_h / 2),
                };
                self.buf().scroll = s.min(total.saturating_sub(view_h)) as u16;
            }
            Parse::SaveQuit => { let _ = self.buf().save(); self.close_current_tab(); }
            Parse::QuitNoSave => { self.close_current_tab(); }
            Parse::Pane(k) => match k {
                'h' => self.move_pane_left(),
                'j' => self.move_pane_down(),
                'k' => self.move_pane_up(),
                _ => self.move_pane_right(),
            },
            Parse::Need | Parse::Bad => {}
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(lines: &[&str], cursor: Pos) -> Buffer {
        let mut b = Buffer::empty();
        b.lines = lines.iter().map(|s| s.to_string()).collect();
        b.cursor = cursor;
        b
    }

    /// Run an operator sequence like "diw" on a buffer; returns (lines, clipboard).
    fn run(lines: &[&str], cursor: Pos, seq: &str, count: usize) -> (Vec<String>, Option<String>, bool) {
        let mut b = buf(lines, cursor);
        match parse(seq) {
            Parse::Op(op, t) => {
                let range = resolve(&b, op, t, count).expect("no range");
                let out = apply(&mut b, op, range);
                (b.lines.clone(), out.text, out.insert)
            }
            other => panic!("{} parsed as {:?}", seq, other),
        }
    }

    #[test]
    fn parse_basics() {
        assert_eq!(parse("g"), Parse::Need);
        assert_eq!(parse("gg"), Parse::FileStart);
        assert_eq!(parse("gx"), Parse::Bad);
        assert_eq!(parse("d"), Parse::Need);
        assert_eq!(parse("di"), Parse::Need);
        assert_eq!(parse("dd"), Parse::Op(Op::Delete, Target::Line));
        assert_eq!(parse("diw"), Parse::Op(Op::Delete, Target::Object(true, 'w')));
        assert_eq!(parse("ci)"), Parse::Op(Op::Change, Target::Object(true, '(')));
        assert_eq!(parse("dib"), Parse::Op(Op::Delete, Target::Object(true, '(')));
        assert_eq!(parse("dix"), Parse::Bad);
        assert_eq!(parse("gUU"), Parse::Op(Op::Upper, Target::Line));
        assert_eq!(parse("guiw"), Parse::Op(Op::Lower, Target::Object(true, 'w')));
        assert_eq!(parse("g~~"), Parse::Op(Op::Toggle, Target::Line));
        assert_eq!(parse(">>"), Parse::Op(Op::Indent, Target::Line));
        assert_eq!(parse("dgg"), Parse::Op(Op::Delete, Target::Motion(Motion::FileStart)));
        assert_eq!(parse("dfx"), Parse::Op(Op::Delete, Target::Find('f', 'x')));
        assert_eq!(parse("fx"), Parse::Find('f', 'x'));
        assert_eq!(parse("r"), Parse::Need);
        assert_eq!(parse("rx"), Parse::Replace('x'));
        assert_eq!(parse("ma"), Parse::SetMark('a'));
        assert_eq!(parse("mA"), Parse::Bad);
        assert_eq!(parse("'a"), Parse::JumpMark(false, 'a'));
        assert_eq!(parse("`a"), Parse::JumpMark(true, 'a'));
        assert_eq!(parse("zz"), Parse::Scroll('z'));
        assert_eq!(parse("ZZ"), Parse::SaveQuit);
        assert_eq!(parse("\u{17}h"), Parse::Pane('h'));
        assert_eq!(parse("\u{17}x"), Parse::Bad);
    }

    #[test]
    fn every_hinted_key_is_a_real_command() {
        // Every row in every hint must lead somewhere valid (never `Bad`).
        let seqs = ["g", "z", "Z", "r", "m", "'", "`", "f", "t", "F", "T", "\u{17}",
                    "d", "y", "c", ">", "<", "gu", "gU", "g~",
                    "di", "da", "dg", "df", "dt", "dF", "dT"];
        for s in seqs {
            let h = hint(s, "").unwrap_or_else(|| panic!("no hint for {:?}", s));
            assert!(!h.rows.is_empty(), "empty hint for {:?}", s);
            for (key, _) in &h.rows {
                // skip descriptive labels that stand for "any char"
                let k: Vec<char> = key.chars().collect();
                let tries: Vec<char> = match key.as_str() {
                    "‹char›" => vec!['x'],
                    "a-z" => vec!['a'],
                    "( b" => vec!['('],
                    "{ B" => vec!['{'],
                    _ => k.clone(),
                };
                let mut seq = s.to_string();
                for c in tries { seq.push(c); }
                assert_ne!(parse(&seq), Parse::Bad, "hint {:?} lists {:?} -> {:?} which is invalid", s, key, seq);
            }
        }
    }

    #[test]
    fn dd_dw_and_counts() {
        let (l, t, _) = run(&["a", "b", "c", "d"], (1, 0), "dd", 0);
        assert_eq!(l, ["a", "c", "d"]);
        assert_eq!(t.unwrap(), "b\n");
        let (l, _, _) = run(&["a", "b", "c", "d"], (0, 0), "dd", 3);
        assert_eq!(l, ["d"]);
        let (l, t, _) = run(&["foo bar baz"], (0, 0), "dw", 0);
        assert_eq!(l, ["bar baz"]);
        assert_eq!(t.unwrap(), "foo ");
        let (l, _, _) = run(&["foo bar baz"], (0, 0), "dw", 2);
        assert_eq!(l, ["baz"]);
        // last word on a line: stop at end of line, do not join
        let (l, _, _) = run(&["foo bar", "next"], (0, 4), "dw", 0);
        assert_eq!(l, ["foo ", "next"]);
        // punctuation is its own word
        let (l, _, _) = run(&["a.b"], (0, 1), "dw", 0);
        assert_eq!(l, ["ab"]);
    }

    #[test]
    fn cw_keeps_trailing_space() {
        let (l, _, ins) = run(&["foo bar"], (0, 0), "cw", 0);
        assert_eq!(l, [" bar"]);
        assert!(ins);
    }

    #[test]
    fn motions() {
        assert_eq!(run(&["foo bar baz"], (0, 8), "db", 0).0, ["foo baz"]);
        assert_eq!(run(&["foo bar baz"], (0, 4), "de", 0).0, ["foo  baz"]);
        assert_eq!(run(&["foo bar baz"], (0, 4), "d$", 0).0, ["foo "]);
        assert_eq!(run(&["foo bar baz"], (0, 4), "d0", 0).0, ["bar baz"]);
        assert_eq!(run(&["  foo bar"], (0, 6), "d^", 0).0, ["  bar"]);
        assert_eq!(run(&["a", "b", "c", "d"], (1, 0), "dj", 0).0, ["a", "d"]);
        assert_eq!(run(&["a", "b", "c", "d"], (2, 0), "dk", 0).0, ["a", "d"]);
        assert_eq!(run(&["a", "b", "c", "d"], (1, 0), "dG", 0).0, ["a"]);
        assert_eq!(run(&["a", "b", "c", "d"], (2, 0), "dgg", 0).0, ["d"]);
        assert_eq!(run(&["a b c d"], (0, 2), "dl", 0).0, ["a  c d"]);
        assert_eq!(run(&["a b c d"], (0, 2), "dh", 0).0, ["ab c d"]);
        assert_eq!(run(&["f(a, b) x"], (0, 1), "d%", 0).0, ["f x"]);
    }

    #[test]
    fn paragraph_motions() {
        let lines = ["one", "two", "", "three"];
        assert_eq!(run(&lines, (0, 0), "d}", 0).0, ["", "three"]);
        assert_eq!(run(&lines, (3, 0), "d{", 0).0, ["one", "two", "three"]);
    }

    #[test]
    fn find_targets() {
        assert_eq!(run(&["a,b,c"], (0, 0), "df,", 0).0, ["b,c"]);
        assert_eq!(run(&["a,b,c"], (0, 0), "dt,", 0).0, [",b,c"]);
        assert_eq!(run(&["a,b,c"], (0, 0), "df,", 2).0, ["c"]);
        assert_eq!(run(&["a,b,c"], (0, 4), "dF,", 0).0, ["a,bc"]);
        assert_eq!(run(&["a,bcd"], (0, 4), "dT,", 0).0, ["a,d"]);
    }

    #[test]
    fn word_objects() {
        assert_eq!(run(&["foo bar baz"], (0, 5), "diw", 0).0, ["foo  baz"]);
        assert_eq!(run(&["foo bar baz"], (0, 5), "daw", 0).0, ["foo baz"]);
        assert_eq!(run(&["foo bar"], (0, 5), "daw", 0).0, ["foo"]); // no trailing space: take leading
        assert_eq!(run(&["foo.bar"], (0, 1), "diw", 0).0, [".bar"]);
        assert_eq!(run(&["foo bar"], (0, 5), "yiw", 0).1.unwrap(), "bar");
    }

    #[test]
    fn quote_objects() {
        let s = r#"say "hi there" ok"#;
        assert_eq!(run(&[s], (0, 7), "di\"", 0).0, [r#"say "" ok"#]);
        assert_eq!(run(&[s], (0, 7), "da\"", 0).0, ["say ok"]);
        assert_eq!(run(&[s], (0, 0), "di\"", 0).0, [r#"say "" ok"#]); // cursor before the quote
        let (l, _, ins) = run(&[s], (0, 7), "ci\"", 0);
        assert_eq!(l, [r#"say "" ok"#]);
        assert!(ins);
        assert_eq!(run(&["a 'b' c"], (0, 3), "di'", 0).0, ["a '' c"]);
        // escaped quote is skipped
        assert_eq!(run(&[r#""a\"b""#], (0, 1), "di\"", 0).0, [r#""""#]);
    }

    #[test]
    fn bracket_objects() {
        assert_eq!(run(&["f(a, (b), c)"], (0, 4), "di(", 0).0, ["f()"]);
        assert_eq!(run(&["f(a, (b), c)"], (0, 7), "di(", 0).0, ["f(a, (), c)"]);
        assert_eq!(run(&["f(a, (b), c)"], (0, 7), "da(", 0).0, ["f(a, , c)"]);
        assert_eq!(run(&["f(a)"], (0, 1), "dib", 0).0, ["f()"]);
        assert_eq!(run(&["f(a)"], (0, 3), "di)", 0).0, ["f()"]); // on the closer
        assert_eq!(run(&["x[1, 2]"], (0, 3), "di]", 0).0, ["x[]"]);
        assert_eq!(run(&["<a href>"], (0, 3), "di<", 0).0, ["<>"]);
        // multi-line block: act on the lines between
        let blk = ["fn f() {", "    a();", "    b();", "}"];
        assert_eq!(run(&blk, (1, 2), "di{", 0).0, ["fn f() {", "}"]);
        let (l, _, ins) = run(&blk, (1, 2), "ci{", 0);
        assert_eq!(l, ["fn f() {", "", "}"]);
        assert!(ins);
        assert_eq!(run(&blk, (1, 2), "da{", 0).0, ["fn f() "]);
    }

    #[test]
    fn paragraph_objects() {
        let lines = ["a", "b", "", "c", "", "d"];
        assert_eq!(run(&lines, (0, 0), "dip", 0).0, ["", "c", "", "d"]);
        assert_eq!(run(&lines, (0, 0), "dap", 0).0, ["c", "", "d"]);
        assert_eq!(run(&lines, (3, 0), "dap", 0).0, ["a", "b", "", "d"]);
    }

    #[test]
    fn yank_and_change_line() {
        let (l, t, _) = run(&["a", "b"], (0, 0), "yy", 0);
        assert_eq!(l, ["a", "b"]);
        assert_eq!(t.unwrap(), "a\n");
        let (l, _, ins) = run(&["a", "b"], (0, 0), "cc", 0);
        assert_eq!(l, ["", "b"]);
        assert!(ins);
    }

    #[test]
    fn indent_and_case() {
        assert_eq!(run(&["a", "b", "c"], (0, 0), ">>", 2).0, ["    a", "    b", "c"]);
        assert_eq!(run(&["    a", "b"], (0, 0), "<<", 0).0, ["a", "b"]);
        assert_eq!(run(&["a", "", "b"], (0, 0), ">ip", 0).0, ["    a", "", "b"]);
        assert_eq!(run(&["foo bar"], (0, 0), "gUiw", 0).0, ["FOO bar"]);
        assert_eq!(run(&["FOO bar"], (0, 0), "guu", 0).0, ["foo bar"]);
        assert_eq!(run(&["Foo Bar"], (0, 0), "g~~", 0).0, ["fOO bAR"]);
        assert_eq!(run(&["foo bar"], (0, 4), "gU$", 0).0, ["foo BAR"]);
    }

    #[test]
    fn ge_goes_to_previous_word_end() {
        let b = buf(&["foo bar"], (0, 5));
        assert_eq!(word_end_back(&b, (0, 5)), (0, 2));
        let b = buf(&["foo", "bar"], (1, 1));
        assert_eq!(word_end_back(&b, (1, 1)), (0, 2));
    }

    #[test]
    fn unicode_is_safe() {
        // Arabic letters count as word characters; no panics on multi-byte text
        assert_eq!(run(&["مرحبا بالعالم"], (0, 0), "diw", 0).0, [" بالعالم"]);
        assert_eq!(run(&["é(à)"], (0, 2), "di(", 0).0, ["é()"]);
    }
}
