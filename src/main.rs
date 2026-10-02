#![allow(dead_code)]

mod app;
mod buffer;
mod config;
mod finder;
mod syntax;
mod terminal;
mod tree;
mod ui;

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
        EnableMouseCapture, Event,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial_file = args.get(1).cloned();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        SetCursorStyle::BlinkingBar,
    )?;

    write!(stdout, "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;
    stdout.flush()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(initial_file);

    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Some(text) = app.take_clipboard_request() {
            let enc = base64_encode(text.as_bytes());
            let mut out = io::stdout();
            let _ = write!(out, "\x1b]52;c;{}\x07", enc);
            let _ = out.flush();
        }

        let poll_ms = if app.has_visible_terminal_pub() { 3 } else { 10 };

        if event::poll(Duration::from_millis(poll_ms))? {
            match event::read()? {
                Event::Key(k) => app.handle_key(k),
                Event::Mouse(m) => app.handle_mouse(m),
                Event::Paste(text) => app.handle_paste(text),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    let mut out = io::stdout();
    let _ = write!(out, "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l");
    let _ = out.flush();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 { out.push(T[((n >> 6) & 63) as usize] as char); }
        else { out.push('='); }
        if chunk.len() > 2 { out.push(T[(n & 63) as usize] as char); }
        else { out.push('='); }
    }
    out
}