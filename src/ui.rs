use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputKind, Mode, Pane};
use crate::config::{Theme, THEME_NAMES};
use crate::finder::FinderKind;
use crate::syntax::{self, Language, TokenKind};
use crate::terminal::Terminal as Term;

// ============================================================
// File icon by extension
// ============================================================
pub fn file_icon(name: &str) -> (&'static str, Color) {
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    if lower == "makefile" { return ("\u{e673}", Color::Rgb(255, 165, 0)); }
    if lower == "dockerfile" { return ("\u{f308}", Color::Rgb(33, 150, 243)); }
    if lower == "license" || lower == "licence" { return ("\u{e60a}", Color::Rgb(200, 200, 100)); }
    if lower == "readme" || lower.starts_with("readme.") { return ("\u{f48a}", Color::Rgb(66, 165, 245)); }
    if lower == ".gitignore" || lower == ".gitmodules" || lower == ".gitattributes" {
        return ("\u{f1d3}", Color::Rgb(240, 80, 50));
    }

    match ext {
        "rs" => ("\u{e7a8}", Color::Rgb(222, 165, 132)),
        "py" | "pyi" => ("\u{e73c}", Color::Rgb(255, 212, 59)),
        "js" | "mjs" | "cjs" => ("\u{e74e}", Color::Rgb(247, 223, 30)),
        "jsx" => ("\u{e7ba}", Color::Rgb(97, 218, 251)),
        "ts" => ("\u{e628}", Color::Rgb(49, 120, 198)),
        "tsx" => ("\u{e7ba}", Color::Rgb(49, 120, 198)),
        "go" => ("\u{e627}", Color::Rgb(81, 205, 228)),
        "c" => ("\u{e61e}", Color::Rgb(85, 148, 207)),
        "h" => ("\u{f0fd}", Color::Rgb(160, 116, 196)),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => ("\u{e61d}", Color::Rgb(243, 75, 125)),
        "java" => ("\u{e738}", Color::Rgb(244, 66, 54)),
        "kt" | "kts" => ("\u{e634}", Color::Rgb(159, 100, 255)),
        "rb" => ("\u{e739}", Color::Rgb(204, 52, 45)),
        "php" => ("\u{e73d}", Color::Rgb(115, 121, 199)),
        "swift" => ("\u{e755}", Color::Rgb(255, 172, 69)),
        "lua" => ("\u{e620}", Color::Rgb(81, 160, 200)),
        "sh" | "bash" | "zsh" | "fish" => ("\u{f489}", Color::Rgb(76, 175, 80)),
        "vim" => ("\u{e62b}", Color::Rgb(120, 190, 80)),
        "sql" => ("\u{e706}", Color::Rgb(218, 165, 32)),
        "asm" | "s" => ("\u{f471}", Color::Rgb(220, 100, 80)),
        "html" | "htm" => ("\u{e736}", Color::Rgb(227, 76, 38)),
        "css" => ("\u{e749}", Color::Rgb(66, 165, 245)),
        "scss" | "sass" => ("\u{e603}", Color::Rgb(236, 117, 165)),
        "less" => ("\u{e758}", Color::Rgb(60, 90, 180)),
        "vue" => ("\u{fd42}", Color::Rgb(65, 184, 131)),
        "svelte" => ("\u{e697}", Color::Rgb(255, 62, 0)),
        "xml" => ("\u{e619}", Color::Rgb(140, 180, 100)),
        "md" | "markdown" => ("\u{e73e}", Color::Rgb(66, 165, 245)),
        "txt" => ("\u{f15c}", Color::Rgb(180, 180, 180)),
        "pdf" => ("\u{f1c1}", Color::Rgb(244, 67, 54)),
        "json" => ("\u{e60b}", Color::Rgb(203, 203, 65)),
        "toml" => ("\u{e615}", Color::Rgb(156, 66, 33)),
        "yaml" | "yml" => ("\u{e615}", Color::Rgb(203, 32, 32)),
        "ini" | "conf" | "cfg" => ("\u{e615}", Color::Rgb(140, 140, 140)),
        "lock" => ("\u{f023}", Color::Rgb(255, 213, 79)),
        "log" => ("\u{f18d}", Color::Rgb(140, 140, 140)),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => {
            ("\u{f1c5}", Color::Rgb(38, 198, 218))
        }
        "mp4" | "mkv" | "avi" | "mov" | "webm" => ("\u{f1c8}", Color::Rgb(171, 71, 188)),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => ("\u{f1c7}", Color::Rgb(233, 30, 99)),
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => {
            ("\u{f410}", Color::Rgb(255, 183, 77))
        }
        "zig" => ("\u{e6a9}", Color::Rgb(247, 164, 29)),
        "nim" => ("\u{e677}", Color::Rgb(255, 198, 84)),
        "hs" | "lhs" => ("\u{e777}", Color::Rgb(94, 80, 134)),
        "ex" | "exs" => ("\u{e62d}", Color::Rgb(126, 71, 158)),
        _ => ("\u{f15b}", Color::Rgb(140, 140, 140)),
    }
}

fn dir_icon(expanded: bool, theme: &Theme) -> (&'static str, Color) {
    if expanded { ("\u{e5fe}", theme.tree_dir) }
    else { ("\u{e5ff}", theme.tree_dir) }
}

fn map_ansi_fg(c: Color, t: &Theme) -> Color {
    match c {
        Color::Reset => t.fg,
        Color::Indexed(i) if i < 8 => match i {
            0 => t.dim, 1 => t.error, 2 => t.tree_dir, 3 => t.search,
            4 => t.tab_active, 5 => t.accent, 6 => t.tree_dir, 7 => t.fg,
            _ => c,
        },
        Color::Indexed(i) if i < 16 => match i - 8 {
            0 => t.dim, 1 => t.error, 2 => t.tree_dir, 3 => t.search,
            4 => t.tab_active, 5 => t.accent, 6 => t.tree_dir, 7 => t.fg,
            _ => c,
        },
        _ => c,
    }
}

fn map_ansi_bg(c: Color, t: &Theme) -> Color {
    match c {
        Color::Reset => t.bg,
        Color::Indexed(i) if i < 8 => match i {
            0 => t.bg, 1 => t.error, 2 => t.tree_dir, 3 => t.search,
            4 => t.tab_active, 5 => t.accent, 6 => t.tree_dir, 7 => t.fg,
            _ => c,
        },
        Color::Indexed(i) if i < 16 => match i - 8 {
            0 => t.dim, 1 => t.error, 2 => t.tree_dir, 3 => t.search,
            4 => t.tab_active, 5 => t.accent, 6 => t.tree_dir, 7 => t.fg,
            _ => c,
        },
        _ => c,
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    app.area_height = size.height;

    {
        let total = app.buf_ref().line_count();
        let view_h = (app.editor_view_h as usize).max(1);
        let max_scroll = total.saturating_sub(view_h) as u16;
        if app.buf_ref().scroll > max_scroll { app.buf().scroll = max_scroll; }
        if total > 0 && app.buf_ref().cursor.0 >= total {
            app.buf().cursor.0 = total - 1;
        }
    }

    app.pump_terminals();
    app.pump_music();
    app.pump_config();
    app.pump_diagnostics();

    let bg = Block::default().style(Style::default().bg(app.theme.bg));
    f.render_widget(bg, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(size);

    let main = chunks[0];
    let status = chunks[1];
    let cmdline = chunks[2];

    let dashboard = app.is_dashboard();
    let show_tree = app.tree.visible && !dashboard;
    let show_side_terminal = !dashboard
        && app.side_term.as_ref().map(|t| t.visible).unwrap_or(false);

    let mut h_constraints: Vec<Constraint> = Vec::new();
    if show_tree { h_constraints.push(Constraint::Length(app.config.tree_width)); }
    h_constraints.push(Constraint::Min(20));
    if show_side_terminal {
        h_constraints.push(Constraint::Length(app.config.term_side_width));
    }

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(h_constraints)
        .split(main);

    let mut i = 0usize;
    let tree_area = if show_tree { let a = h_chunks[i]; i += 1; Some(a) } else { None };
    let editor_col_area = h_chunks[i]; i += 1;
    let side_term_area = if show_side_terminal { Some(h_chunks[i]) } else { None };

    if tree_area.is_none() { app.last_tree_area = Rect::default(); }
    if side_term_area.is_none() { app.last_side_term_area = Rect::default(); }

    let show_bottom = !dashboard
        && app.bottom_term.as_ref().map(|t| t.visible).unwrap_or(false);

    let (editor_area, bottom_term_area) = if show_bottom {
        let h = app.config.term_bottom_height;
        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(h)])
            .split(editor_col_area);
        (v[0], Some(v[1]))
    } else {
        app.last_bottom_term_area = Rect::default();
        (editor_col_area, None)
    };

    if let Some(ta) = tree_area { draw_tree(f, app, ta); }

    if !dashboard {
        draw_editor(f, app, editor_area);
    } else {
        let bg_block = Block::default().style(Style::default().bg(app.theme.bg));
        f.render_widget(bg_block, editor_area);
        app.last_editor_area = editor_area;
        app.last_text_area = Rect::default();
        app.last_tabs_area = Rect::default();
        app.cursor_screen_pos = (0, 0);
    }

    if let Some(ta) = bottom_term_area { draw_bottom_term(f, app, ta); }
    if let Some(ta) = side_term_area { draw_side_term(f, app, ta); }

    draw_status(f, app, status);
    draw_cmdline(f, app, cmdline);

    if dashboard { draw_dashboard(f, app, editor_col_area); }
    if app.completion_active { draw_completion(f, app, size); }
    if app.leader { draw_whichkey(f, app, size); }
    if app.finder.visible { draw_finder(f, app, size); }
    if app.theme_picker_visible { draw_theme_picker(f, app, size); }
    if app.show_help { draw_help(f, app, size); }
    if app.input_kind != InputKind::None { draw_input_popup(f, app, size); }
    if app.confirm_delete.is_some() { draw_confirm(f, app, size); }
}

// ---------------- TERMINAL ----------------
fn draw_terminal_grid(
    f: &mut Frame, theme: &Theme, area: Rect, term: &Term,
    title: &str, focused: bool,
) {
    let border = if focused { theme.accent } else { theme.border };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(theme.bg))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let draw_w = (inner.width as usize).min(term.width);
    let draw_h = (inner.height as usize).min(term.height);
    let total_w = inner.width as usize;

    let mut lines: Vec<Line> = Vec::new();
    for y in 0..draw_h {
        let mut spans: Vec<Span> = Vec::new();
        for x in 0..draw_w {
            let cell = &term.grid[y * term.width + x];
            let fg = map_ansi_fg(cell.fg, theme);
            let bg = map_ansi_bg(cell.bg, theme);
            let mut st = Style::default().fg(fg).bg(bg);
            if cell.bold { st = st.add_modifier(Modifier::BOLD); }
            spans.push(Span::styled(cell.ch.to_string(), st));
        }
        let pad = total_w.saturating_sub(draw_w);
        if pad > 0 {
            spans.push(Span::styled(" ".repeat(pad), Style::default().bg(theme.bg)));
        }
        lines.push(Line::from(spans));
    }
    while lines.len() < inner.height as usize {
        lines.push(Line::from(""));
    }
    f.render_widget(Paragraph::new(lines).style(Style::default().bg(theme.bg)), inner);

    if focused && term.cursor_visible {
        let cx = inner.x + (term.cursor_x as u16).min(inner.width.saturating_sub(1));
        let cy = inner.y + (term.cursor_y as u16).min(inner.height.saturating_sub(1));
        f.set_cursor_position((cx, cy));
    }
}

fn draw_side_term(f: &mut Frame, app: &mut App, area: Rect) {
    app.last_side_term_area = area;
    let focused = app.active_pane == Pane::SideTerm;
    let inner_w = area.width.saturating_sub(2).max(1);
    let inner_h = area.height.saturating_sub(2).max(1);
    let theme = app.theme.clone();
    let Some(t) = app.side_term.as_mut() else { return; };
    if t.width != inner_w as usize || t.height != inner_h as usize {
        t.resize(inner_h, inner_w);
    }
    draw_terminal_grid(f, &theme, area, t, "\u{f489} shell >", focused);
}

fn draw_bottom_term(f: &mut Frame, app: &mut App, area: Rect) {
    app.last_bottom_term_area = area;
    let focused = app.active_pane == Pane::BottomTerm;
    let inner_w = area.width.saturating_sub(2).max(1);
    let inner_h = area.height.saturating_sub(2).max(1);
    let theme = app.theme.clone();
    let Some(t) = app.bottom_term.as_mut() else { return; };
    if t.width != inner_w as usize || t.height != inner_h as usize {
        t.resize(inner_h, inner_w);
    }
    draw_terminal_grid(f, &theme, area, t, "\u{f489} shell v", focused);
}

// ---------------- TREE ----------------
fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    app.last_tree_area = area;
    let theme = app.theme.clone();
    let focused = app.active_pane == Pane::Tree || app.tree_focus;

    let border_color = if focused { theme.accent } else { theme.border };
    let title_color = if focused { theme.accent } else { theme.fg };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg))
        .title(Span::styled(
            "  \u{f07b}  EXPLORER  ",
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let list_h = inner.height.saturating_sub(1);
    let list_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h };
    let bc_area = Rect { x: inner.x, y: inner.y + list_h, width: inner.width, height: 1 };

    let scroll = app.tree.scroll as usize;
    let view_h = list_h as usize;

    let items: Vec<ListItem> = app.tree.entries.iter().skip(scroll).take(view_h)
        .map(|e| {
            let indent = " ".repeat(e.depth * 2);
            let (icon, icon_color) = if e.is_dir {
                dir_icon(e.expanded, &theme)
            } else {
                file_icon(&e.name)
            };
            let chevron = if e.is_dir {
                if e.expanded { "\u{f078} " } else { "\u{f054} " }
            } else { "  " };
            Line::from(vec![
                Span::raw(indent),
                Span::styled(chevron, Style::default().fg(theme.dim)),
                Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
                Span::styled(e.name.clone(), Style::default().fg(theme.fg)),
            ]).into()
        })
        .collect();

    let selected_in_view = app.tree.selected.saturating_sub(scroll);
    let list = List::new(items).highlight_style(
        Style::default().bg(theme.visual).add_modifier(Modifier::BOLD),
    );

    let mut state = ratatui::widgets::ListState::default();
    if app.tree.selected < app.tree.entries.len() {
        state.select(Some(selected_in_view));
    }
    f.render_stateful_widget(list, list_area, &mut state);

    let bc_line = Line::from(Span::styled(
        app.tree.breadcrumb(),
        Style::default().fg(theme.dim),
    ));
    f.render_widget(Paragraph::new(bc_line), bc_area);
}

// ---------------- EDITOR ----------------
fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.clone();
    app.last_editor_area = area;

    let tabs_h: u16 = if app.buffers.len() > 1 { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(tabs_h), Constraint::Min(1)])
        .split(area);

    let (tabs_area, text_area) = (chunks[0], chunks[1]);
    app.last_text_area = text_area;

    if tabs_h > 0 {
        app.last_tabs_area = tabs_area;
        draw_tabs(f, app, tabs_area);
    } else {
        app.last_tabs_area = Rect::default();
    }

    let total_lines = app.buf_ref().line_count();
    let num_width = total_lines.to_string().len().max(3) as u16;
    let diag_col_width: u16 = 2;
    let num_col_width = num_width + 2 + diag_col_width;

    let view_h = text_area.height as usize;
    app.editor_view_h = text_area.height;

    {
        let max_scroll = total_lines.saturating_sub(view_h) as u16;
        if app.buf_ref().scroll > max_scroll {
            app.buf().scroll = max_scroll;
        }
    }

    let filename = app.buf_ref().filename();
    let lang = syntax::detect(&filename);

    let start = (app.buf_ref().scroll as usize).min(total_lines.saturating_sub(1));
    let cursor = app.buf_ref().cursor;
    let anchor = app.buf_ref().visual_anchor;
    let mode_visual = matches!(app.mode, Mode::Visual | Mode::VisualLine);
    let search_hits = app.search_hits.clone();
    let search_len = app.search.as_ref().map(|s| s.chars().count()).unwrap_or(0);

    let diag_map: std::collections::HashMap<usize, String> = app.diagnostics.iter()
        .map(|(li, msg)| (*li, msg.clone()))
        .collect();

    let bg_block = Block::default().style(Style::default().bg(theme.bg));
    f.render_widget(bg_block, text_area);

    let text_x = text_area.x + num_col_width;
    let content_w = text_area.width.saturating_sub(num_col_width) as usize;

    let mut lines: Vec<Line> = Vec::new();
    for i in start..(start + view_h).min(total_lines) {
        let raw = app.buf_ref().line(i).to_string();
        let chars: Vec<char> = raw.chars().collect();
        let tokens = syntax::highlight(&raw, lang);
        let is_cursor_line = i == cursor.0;
        let has_error = diag_map.contains_key(&i);

        let line_bg = if is_cursor_line { theme.cursor_line } else { theme.bg };

        let err_marker = if has_error { "\u{f071} " } else { "  " };
        let err_style = Style::default()
            .fg(theme.error)
            .bg(line_bg)
            .add_modifier(Modifier::BOLD);

        let num_style = if is_cursor_line {
            Style::default()
                .fg(theme.accent)
                .bg(line_bg)
                .add_modifier(Modifier::BOLD)
        } else if has_error {
            Style::default()
                .fg(theme.error)
                .bg(line_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim).bg(line_bg)
        };
        let num_text = format!("{:>width$}  ", i + 1, width = num_width as usize);

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(err_marker, err_style));
        spans.push(Span::styled(num_text, num_style));

        let vis_start = if mode_visual {
            anchor.map(|a| if (a.0, a.1) <= cursor { a } else { cursor })
        } else { None };
        let vis_end = if mode_visual {
            anchor.map(|a| if (a.0, a.1) <= cursor { cursor } else { a })
        } else { None };

        let line_hits: Vec<usize> = search_hits.iter()
            .filter(|(li, _)| *li == i)
            .map(|(_, c)| *c)
            .collect();

        let mut col = 0usize;
        while col < chars.len() {
            let mut style = Style::default().fg(theme.fg).bg(line_bg);
            let in_visual = match (vis_start, vis_end) {
                (Some((sl, sc)), Some((el, ec))) => {
                    if i > sl && i < el { true }
                    else if i == sl && i == el { col >= sc && col <= ec }
                    else if i == sl { col >= sc }
                    else if i == el { col <= ec }
                    else { false }
                }
                _ => false,
            };
            let is_search = search_len > 0
                && line_hits.iter().any(|hc| *hc <= col && col < hc + search_len);

            if is_search {
                style = style.bg(theme.search).fg(theme.bg);
            } else {
                let kind = tokens.get(col).copied().unwrap_or(TokenKind::Normal);
                style = style.fg(syntax::token_color(kind, &theme));
                if in_visual { style = style.bg(theme.visual); }
            }

            spans.push(Span::styled(chars[col].to_string(), style));
            col += 1;
        }

        if is_cursor_line && chars.len() < content_w {
            let pad = content_w - chars.len();
            spans.push(Span::styled(
                " ".repeat(pad),
                Style::default().bg(line_bg),
            ));
        }

        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    f.render_widget(paragraph, text_area);

    if !matches!(app.mode, Mode::Command) && app.active_pane == Pane::Editor {
        let cur_line = cursor.0;
        let cur_col = cursor.1;
        if cur_line >= start && cur_line < start + view_h {
            let y = text_area.y + (cur_line - start) as u16;
            let x = text_x + (cur_col as u16);
            app.cursor_screen_pos = (x, y);
            if x < text_area.x + text_area.width
                && y < text_area.y + text_area.height
            {
                f.set_cursor_position((x, y));
            }
        }
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let mut spans: Vec<Span> = Vec::new();
    let total = app.buffers.len();
    let mut visual_idx = 0usize;
    for (i, b) in app.buffers.iter().enumerate() {
        let is_scratch = b.path.is_none()
            && !b.modified
            && b.lines.len() == 1
            && b.lines[0].is_empty();
        if is_scratch && total > 1 { continue; }

        let mut name = b.filename();
        if b.modified { name.push_str(" ●"); }
        let (icon, icon_color) = file_icon(&name);
        let is_active = i == app.current;

        if visual_idx > 0 {
            spans.push(Span::styled(
                " ",
                Style::default().fg(theme.border).bg(theme.status_bg),
            ));
        }

        let bg = if is_active { theme.visual } else { theme.status_bg };
        let fg = if is_active { theme.fg } else { theme.dim };

        spans.push(Span::styled(
            format!(" {} ", icon),
            Style::default().fg(icon_color).bg(bg),
        ));
        spans.push(Span::styled(
            format!("{} ", name),
            Style::default()
                .fg(fg)
                .bg(bg)
                .add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() }),
        ));
        spans.push(Span::styled(
            " │ ",
            Style::default().fg(theme.border).bg(theme.status_bg),
        ));

        visual_idx += 1;
    }
    let p = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.status_bg));
    f.render_widget(p, area);
}

// ---------------- STATUS ----------------
fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let b = app.buf_ref();

    let mode_color = match app.mode {
        Mode::Normal => theme.accent,
        Mode::Insert => theme.tree_dir,
        Mode::Visual | Mode::VisualLine => theme.error,
        Mode::Command => theme.search,
    };

    let pane_label = match app.active_pane {
        Pane::Tree => " \u{f07b} TREE ",
        Pane::SideTerm => " \u{f120} TERM ",
        Pane::BottomTerm => " \u{f120} TERM ",
        Pane::Editor => "",
    };

    let left_mode = format!(" {} ", app.mode_str());
    let mut filename = b.filename();
    if b.modified { filename.push_str(" ●"); }
    let lang = syntax::detect(&filename);
    let lang_label = if matches!(lang, Language::None) {
        String::new()
    } else {
        format!(" {} ", lang.name())
    };
    let pos = format!(" {}:{} ", b.cursor.0 + 1, b.cursor.1 + 1);

    let mut left_spans = vec![
        Span::styled(left_mode,
            Style::default().bg(mode_color).fg(theme.bg).add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::default().bg(theme.status_bg)),
    ];
    if !lang_label.is_empty() {
        left_spans.push(Span::styled(lang_label,
            Style::default().bg(theme.tree_dir).fg(theme.bg).add_modifier(Modifier::BOLD)));
        left_spans.push(Span::styled(" ", Style::default().bg(theme.status_bg)));
    }
    if !pane_label.is_empty() {
        left_spans.push(Span::styled(pane_label,
            Style::default().bg(theme.tab_active).fg(theme.bg).add_modifier(Modifier::BOLD)));
        left_spans.push(Span::styled(" ", Style::default().bg(theme.status_bg)));
    }
    left_spans.push(Span::styled(filename,
        Style::default().fg(theme.status_fg).bg(theme.status_bg)));
    left_spans.push(Span::styled("│",
        Style::default().fg(theme.border).bg(theme.status_bg)));
    left_spans.push(Span::styled(pos, Style::default().fg(theme.dim).bg(theme.status_bg)));

    if !app.diagnostics.is_empty() {
        left_spans.push(Span::styled(
            format!(" ✗{} ", app.diagnostics.len()),
            Style::default().bg(theme.error).fg(theme.bg).add_modifier(Modifier::BOLD),
        ));
    }

    let left = Line::from(left_spans);
    let p = Paragraph::new(left).style(Style::default().bg(theme.status_bg));
    f.render_widget(p, area);

    let music_part = if app.config.music_enabled && !app.music_title.is_empty() {
        let short: String = app.music_title.chars().take(20).collect();
        let truncated = if app.music_title.chars().count() > 20 {
            format!("{}…", short)
        } else { short };
        format!("\u{f001} {}  │  ", truncated)
    } else { String::new() };

    let right = format!("   {}{} │ utf-8 │ {} ",
        music_part, theme.name, app.elapsed_str());
    let right_l = Line::from(Span::styled(right,
        Style::default().fg(theme.dim).bg(theme.status_bg)))
        .alignment(Alignment::Right);
    let rp = Paragraph::new(right_l).style(Style::default().bg(theme.status_bg));
    let rw = 70u16.min(area.width);
    let right_area = Rect {
        x: area.x + area.width.saturating_sub(rw),
        y: area.y, width: rw, height: 1,
    };
    f.render_widget(rp, right_area);
}

fn draw_cmdline(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let (text, style) = match app.mode {
        Mode::Command => (
            format!("{}{}", app.command_prefix, app.command),
            Style::default().fg(theme.fg).bg(theme.bg),
        ),
        _ => {
            let cursor_line = app.buf_ref().cursor.0;
            let err_msg = app.diagnostics.iter()
                .find(|(li, _)| *li == cursor_line)
                .map(|(_, m)| m.clone());
            if let Some(msg) = err_msg {
                (format!("✗ {}", msg), Style::default().fg(theme.error).bg(theme.bg))
            } else if !app.message.is_empty() {
                (app.message.clone(), Style::default().fg(theme.dim).bg(theme.bg))
            } else {
                (String::new(), Style::default().bg(theme.bg))
            }
        }
    };
    let p = Paragraph::new(text).style(style);
    f.render_widget(p, area);
}

// ---------------- DASHBOARD ----------------
fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let logo = &app.config.logo;
    let tagline = &app.config.tagline;

    let rows = (app.config.menu.len() + 1) / 2;
    let content_h = logo.len() + 6 + rows + 3;
    let mut lines: Vec<Line> = Vec::new();
    let h = area.height as usize;
    let pad_top = if h > content_h { (h - content_h) / 2 } else { 0 };

    for _ in 0..pad_top { lines.push(Line::raw("")); }
    for l in logo {
        lines.push(Line::from(Span::styled(l.clone(),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(tagline.clone(),
        Style::default().fg(theme.dim).add_modifier(Modifier::ITALIC))));
    lines.push(Line::raw(""));

    let sep_w = 44usize.min(area.width as usize).saturating_sub(4);
    lines.push(Line::from(Span::styled(
        "─".repeat(sep_w),
        Style::default().fg(theme.border),
    )));
    lines.push(Line::raw(""));

    let items = &app.config.menu;
    let half = (items.len() + 1) / 2;
    for row in 0..half {
        let left = items.get(row);
        let right = items.get(row + half);
        let mut spans: Vec<Span> = Vec::new();
        spans.extend(menu_cell(left, theme));
        spans.push(Span::raw("   "));
        spans.extend(menu_cell(right, theme));
        lines.push(Line::from(spans));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("\u{f017} ", Style::default().fg(theme.dim)),
        Span::styled(format!("anvil v1.0.0  ·  {}", app.elapsed_str()),
            Style::default().fg(theme.dim)),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("press ", Style::default().fg(theme.dim)),
        Span::styled("Space", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(" for leader  ·  ", Style::default().fg(theme.dim)),
        Span::styled("?", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(" for help", Style::default().fg(theme.dim)),
    ]));

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(theme.bg));
    f.render_widget(p, area);
}

fn menu_cell<'a>(
    item: Option<&'a crate::config::MenuItem>,
    theme: &Theme,
) -> Vec<Span<'a>> {
    match item {
        Some(it) => vec![
            Span::styled(
                format!(" {} ", it.key),
                Style::default().fg(theme.bg).bg(theme.accent)
                    .add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                format!("{} ", it.icon),
                Style::default().fg(theme.tree_dir).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:<14}", it.text),
                Style::default().fg(theme.fg)),
        ],
        None => vec![Span::raw(" ".repeat(22))],
    }
}

// ---------------- COMPLETION ----------------
fn draw_completion(f: &mut Frame, app: &App, size: Rect) {
    let theme = &app.theme;
    let (cx, cy) = app.cursor_screen_pos;
    let count = app.completion_items.len().min(10) as u16;
    if count == 0 { return; }

    let name_max: usize = 40;
    let widest = app.completion_items.iter().take(count as usize)
        .map(|s| s.chars().count().min(name_max)).max().unwrap_or(10) as u16;
    let w: u16 = (widest + 8).min(size.width.saturating_sub(4)).max(20);
    let h = count + 2;

    let mut x = cx;
    if x + w > size.width { x = size.width.saturating_sub(w); }
    let mut y = cy.saturating_add(1);
    if y + h > size.height.saturating_sub(2) { y = cy.saturating_sub(h); }
    if y + h > size.height.saturating_sub(2) { y = size.height.saturating_sub(h + 2); }
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled("  \u{f121}  Completion  ",
            Style::default().fg(theme.bg).bg(theme.accent)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let row_w = inner.width as usize;
    let items: Vec<ListItem> = app.completion_items.iter().take(count as usize).enumerate()
        .map(|(i, s)| {
            let active = i == app.completion_index;
            let (icon, icon_color) = file_icon(s);
            let bg = if active { theme.visual } else { theme.bg };
            let icon_fg = if active { theme.accent } else { icon_color };
            let bold = if active { Modifier::BOLD } else { Modifier::empty() };

            let mut name = s.clone();
            if name.chars().count() > name_max {
                name = name.chars().take(name_max - 1).collect();
                name.push('…');
            }
            let icon_part = format!(" {}  ", icon);
            let used = icon_part.chars().count() + name.chars().count();
            let pad = row_w.saturating_sub(used);

            ListItem::new(Line::from(vec![
                Span::styled(icon_part, Style::default().fg(icon_fg).bg(bg).add_modifier(bold)),
                Span::styled(name, Style::default().fg(theme.fg).bg(bg).add_modifier(bold)),
                Span::styled(" ".repeat(pad), Style::default().bg(bg)),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), inner);
}

// ---------------- WHICH-KEY ----------------
fn draw_whichkey(f: &mut Frame, app: &mut App, size: Rect) {
    let theme = app.theme.clone();
    let bindings: Vec<(&str, &str)> = vec![
        ("a", "select all"),
        ("c", "copy"),
        ("w", "save file"),
        ("r", "run file"),
        ("s", "toggle tree / editor"),
        ("e", "toggle tree"),
        ("v", "side terminal"),
        ("j", "bottom terminal"),
        ("f", "find file"),
        ("F", "find (home)"),
        ("g", "live grep"),
        ("l", "goto line"),
        ("n", "new file"),
        ("t", "theme picker"),
        ("h", "help"),
        ("q", "close tab"),
        ("Q", "force quit"),
    ];
    let w: u16 = 42;
    let h: u16 = (bindings.len() as u16) + 2;
    let x = size.width.saturating_sub(w + 2);
    let y = if size.height > h + 4 { size.height.saturating_sub(h + 4) } else { 0 };
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled("  \u{f0e7}  LEADER  ",
            Style::default().fg(theme.bg).bg(theme.accent)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    app.last_leader_area = popup;

    let items: Vec<ListItem> = bindings.iter()
        .map(|(k, v)| {
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!(" {} ", k),
                    Style::default()
                        .fg(theme.bg)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(*v, Style::default().fg(theme.fg)),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), inner);
}

// ---------------- THEME PICKER ----------------
fn draw_theme_picker(f: &mut Frame, app: &mut App, size: Rect) {
    let theme = app.theme.clone();
    let total = THEME_NAMES.len();
    let list_h = total as u16 + 2;
    let w = 44u16.min(size.width.saturating_sub(4));
    let h = list_h.min(size.height.saturating_sub(4));
    let x = (size.width - w) / 2;
    let y = (size.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            "  \u{e22b}  Themes  ",
            Style::default()
                .fg(theme.bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let view_h = inner.height as usize;
    let sel = app.theme_picker_index;
    let scroll = if sel >= view_h { sel + 1 - view_h } else { 0 };

    app.last_theme_list_area = inner;

    let items: Vec<ListItem> = THEME_NAMES
        .iter()
        .enumerate()
        .skip(scroll)
        .take(view_h)
        .map(|(i, name)| {
            let active = i == app.theme_picker_index;
            let is_current = *name == app.theme.name;

            // Marker + name, both in the CURRENT theme's colors.
            let marker = if is_current { " * " } else { "   " };
            let marker_color = if is_current { theme.tree_dir } else { theme.dim };

            let text = format!("{:<30}", name);
            let text_color = if active { theme.bg } else { theme.fg };

            let trailing = if is_current { " current " } else { "" };
            let trail_color = if active { theme.bg } else { theme.dim };

            let bg = if active { theme.accent } else { theme.bg };

            ListItem::new(Line::from(vec![
                Span::styled(marker,
                    Style::default().fg(marker_color).bg(bg)),
                Span::styled(text,
                    Style::default()
                        .fg(text_color)
                        .bg(bg)
                        .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() })),
                Span::styled(trailing,
                    Style::default().fg(trail_color).bg(bg)),
            ]))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

// ---------------- FINDER ----------------
fn draw_finder(f: &mut Frame, app: &mut App, size: Rect) {
    let theme = app.theme.clone();
    let w = (size.width * 4 / 5).max(40).min(size.width);
    let h = (size.height * 4 / 5).max(10).min(size.height);
    let x = (size.width - w) / 2;
    let y = (size.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, popup);

    let (title, title_color) = match app.finder.kind {
        FinderKind::File => ("  \u{f002}  Find File  ", theme.accent),
        FinderKind::Grep => ("  \u{f002}  Live Grep  ", theme.tree_dir),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(title_color))
        .title(Span::styled(title,
            Style::default().fg(theme.bg).bg(title_color)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let qline = Paragraph::new(Line::from(vec![
        Span::styled(" \u{f054} ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(app.finder.query.clone(), Style::default().fg(theme.fg)),
        Span::styled(
            if app.finder.query.is_empty() { "type to filter…" } else { "" },
            Style::default().fg(theme.dim)),
    ]));
    f.render_widget(qline, chunks[0]);

    let sep = Paragraph::new("─".repeat(inner.width as usize))
        .style(Style::default().fg(theme.border));
    f.render_widget(sep, chunks[1]);

    app.last_finder_list_area = chunks[2];

    match app.finder.kind {
        FinderKind::File => {
            let items: Vec<ListItem> = app.finder.file_results.iter()
                .take(chunks[2].height as usize)
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let (icon, icon_color) = file_icon(&name);
                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", icon), Style::default().fg(icon_color)),
                        Span::styled(s, Style::default().fg(theme.fg)),
                    ]))
                })
                .collect();
            let list = List::new(items).highlight_style(
                Style::default().bg(theme.accent).fg(theme.bg).add_modifier(Modifier::BOLD));
            let mut state = ratatui::widgets::ListState::default();
            state.select(Some(app.finder.selected));
            f.render_stateful_widget(list, chunks[2], &mut state);
        }
        FinderKind::Grep => {
            let items: Vec<ListItem> = app.finder.grep_results.iter()
                .take(chunks[2].height as usize)
                .map(|h| ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}:{} ", h.path.display(), h.line),
                        Style::default().fg(theme.accent)),
                    Span::styled(h.text.clone(), Style::default().fg(theme.fg)),
                ])))
                .collect();
            let list = List::new(items).highlight_style(
                Style::default().bg(theme.accent).fg(theme.bg).add_modifier(Modifier::BOLD));
            let mut state = ratatui::widgets::ListState::default();
            state.select(Some(app.finder.selected));
            f.render_stateful_widget(list, chunks[2], &mut state);
        }
    }
}

// ---------------- INPUT POPUP ----------------
fn draw_input_popup(f: &mut Frame, app: &App, size: Rect) {
    let theme = &app.theme;
    let (label, hint) = match app.input_kind {
        InputKind::NewFile => ("  \u{f15b}  New File  ", "path/filename.ext"),
        InputKind::GotoLine => ("  \u{f0da}  Goto Line  ", "line number"),
        InputKind::SaveAs => ("  \u{f0c7}  Save As  ", "path"),
        InputKind::TreeNewFile => ("  \u{f15b}  New File  ", "filename"),
        InputKind::TreeNewFolder => ("  \u{f07b}  New Folder  ", "folder name"),
        InputKind::None => return,
    };
    let w = 64u16.min(size.width.saturating_sub(4));
    let h = 4;
    let x = (size.width - w) / 2;
    let y = (size.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(label,
            Style::default().fg(theme.bg).bg(theme.accent)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let input_line = Line::from(vec![
        Span::styled(" \u{f054} ",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(app.input_text.clone(), Style::default().fg(theme.fg)),
        Span::styled(
            if app.input_text.is_empty() { hint } else { "" },
            Style::default().fg(theme.dim)),
    ]);
    f.render_widget(Paragraph::new(input_line), inner);

    let cx = inner.x + 3 + app.input_text.chars().count() as u16;
    let cy = inner.y;
    f.set_cursor_position((cx, cy));
}

// ---------------- CONFIRM DELETE ----------------
fn draw_confirm(f: &mut Frame, app: &App, size: Rect) {
    let theme = &app.theme;
    let path = match &app.confirm_delete { Some(p) => p, None => return };
    let name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let w = 60u16.min(size.width.saturating_sub(4));
    let h = 5;
    let x = (size.width - w) / 2;
    let y = (size.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.error))
        .title(Span::styled("  \u{f1f8}  Confirm Delete  ",
            Style::default().fg(theme.bg).bg(theme.error)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = vec![
        Line::from(vec![
            Span::styled("Delete ", Style::default().fg(theme.fg)),
            Span::styled(name.clone(),
                Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled(" ?", Style::default().fg(theme.fg)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [y] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("yes    ", Style::default().fg(theme.fg)),
            Span::styled("[n] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("no", Style::default().fg(theme.fg)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).style(Style::default().bg(theme.bg)), inner);
}

// ---------------- HELP ----------------
fn draw_help(f: &mut Frame, app: &App, size: Rect) {
    let theme = &app.theme;
    let w = (size.width * 3 / 4).max(50).min(size.width);
    let h = (size.height * 3 / 4).max(15).min(size.height);
    let x = (size.width - w) / 2;
    let y = (size.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled("  \u{f059}  Help  ",
            Style::default().fg(theme.bg).bg(theme.accent)
                .add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let head = |s: &'static str| {
        Line::from(Span::styled(s,
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)))
    };

    let text = vec![
        head("LEADER (Space)"),
        Line::raw("  Space a        select all"),
        Line::raw("  Space c        copy"),
        Line::raw("  Space w        save"),
        Line::raw("  Space r        run file"),
        Line::raw("  Space s        toggle tree/editor focus"),
        Line::raw("  Space e        toggle tree visibility"),
        Line::raw("  Space v j      side term / bottom term"),
        Line::raw("  Space q Q      close tab / force quit"),
        Line::raw("  Space f F g    find file / home / grep"),
        Line::raw("  Space n t l h  new / theme / goto / help"),
        Line::raw(""),
        head("RUN FILE (Space+r or :run)"),
        Line::raw("  Detects language from the file's extension:"),
        Line::raw("    .py → python3    .js → node"),
        Line::raw("    .rs → rustc      .go → go run"),
        Line::raw("    .c/.cpp → gcc/g++ → run"),
        Line::raw("    .sh → bash       .rb → ruby"),
        Line::raw(""),
        head("TREE"),
        Line::raw("  j k  ↑↓ Tab    navigate"),
        Line::raw("  Enter / l      open or expand"),
        Line::raw("  h              collapse"),
        Line::raw("  a / f          new file / folder"),
        Line::raw("  d              delete"),
        Line::raw("  Backspace      go back (history)"),
        Line::raw("  -              go up (parent dir)"),
        Line::raw("  H / ~          go home"),
        Line::raw("  R              refresh"),
        Line::raw(""),
        head("MOUSE"),
        Line::raw("  • finder / theme: click select, double-click open"),
        Line::raw("  • scroll works everywhere"),
        Line::raw("  • click editor to move cursor, drag to select"),
        Line::raw(""),
        Line::raw("Press any key to close."),
    ];

    let p = Paragraph::new(text)
        .style(Style::default().bg(theme.bg).fg(theme.fg))
        .wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}
