//! Toggleterm-style floating terminal: a shell in a PTY, parsed by vt100.

use std::io::{Read, Write};
use std::thread;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::draw::float_with;
use super::theme::Theme;
use crate::event::{AppEvent, TermSlot, Tx};

const SCROLLBACK: usize = 5000;

pub struct Terminal {
    parser: vt100::Parser,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    size: (u16, u16),
    title: String,
    /// The start of a possible query split across reads (see [`scan`]).
    held: Vec<u8>,
}

impl Terminal {
    /// Runs `command` (or the user's shell) in a new pseudo-terminal, with
    /// environment `env`.
    pub fn spawn(
        rows: u16,
        cols: u16,
        command: Option<&[String]>,
        env: &crate::projenv::Env,
        slot: TermSlot,
        tx: Tx,
    ) -> Result<Self> {
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        let pair = native_pty_system().openpty(size)?;
        let (mut cmd, title) = match command {
            Some([prog, args @ ..]) => {
                let mut cmd = CommandBuilder::new(prog);
                cmd.args(args);
                (cmd, prog.rsplit('/').next().unwrap_or(prog).to_owned())
            }
            _ => (CommandBuilder::new_default_prog(), "Terminal".to_owned()),
        };
        cmd.env_clear();
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(AppEvent::PtyOutput(slot, buf[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(AppEvent::PtyExited(slot));
        });
        Ok(Self {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK),
            master: pair.master,
            writer,
            child,
            size: (rows, cols),
            title,
            held: Vec::new(),
        })
    }

    /// Whether something other than the shell is running in the foreground
    /// (a build, a server, an editor).
    pub fn busy(&self) -> bool {
        match (self.master.process_group_leader(), self.child.process_id()) {
            (Some(leader), Some(pid)) => leader as u32 != pid,
            _ => false,
        }
    }

    /// Feeds program output to the screen, answering the queries programs
    /// wait on (fish 4 stalls at startup without a device-attributes reply).
    pub fn process(&mut self, bytes: &[u8]) {
        let mut buf = std::mem::take(&mut self.held);
        buf.extend_from_slice(bytes);
        let (queries, held) = scan(&buf);
        let mut done = 0;
        for (end, query) in queries {
            // Answer with the screen as it is right after the query.
            self.parser.process(&buf[done..end]);
            done = end;
            let reply = match query {
                Query::PrimaryAttributes => b"\x1b[?62;22c".to_vec(),
                Query::SecondaryAttributes => b"\x1b[>1;10;0c".to_vec(),
                Query::Status => b"\x1b[0n".to_vec(),
                Query::Cursor => {
                    let (row, col) = self.parser.screen().cursor_position();
                    format!("\x1b[{};{}R", row + 1, col + 1).into_bytes()
                }
            };
            self.write(&reply);
        }
        self.parser.process(&buf[done..held]);
        self.held = buf[held..].to_vec();
    }

    fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) != self.size && rows > 0 && cols > 0 {
            self.size = (rows, cols);
            self.parser.screen_mut().set_size(rows, cols);
            let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        let screen = self.parser.screen_mut();
        let cur = screen.scrollback() as isize;
        screen.set_scrollback((cur + delta).max(0) as usize);
    }

    pub fn key(&mut self, key: &KeyEvent) {
        // Typing jumps back to the live screen.
        self.parser.screen_mut().set_scrollback(0);
        let app_cursor = self.parser.screen().application_cursor();
        if let Some(bytes) = encode_key(key, app_cursor) {
            self.write(&bytes);
        }
    }

    pub fn paste(&mut self, text: &str) {
        if self.parser.screen().bracketed_paste() {
            self.write(format!("\x1b[200~{text}\x1b[201~").as_bytes());
        } else {
            self.write(text.as_bytes());
        }
    }

    /// The float's frame for a given screen.
    pub fn area(screen: Rect) -> Rect {
        let w = (screen.width * 9 / 10).max(20.min(screen.width));
        let h = (screen.height * 8 / 10).max(5.min(screen.height));
        Rect::new(screen.x + (screen.width - w) / 2, screen.y + (screen.height - h) / 2, w, h)
    }

    pub fn render(&mut self, buf: &mut Buffer, screen: Rect, theme: &Theme) {
        let area = Self::area(screen);
        let scrolled = self.parser.screen().scrollback();
        let title = if scrolled > 0 { format!("{} [+{scrolled}]", self.title) } else { self.title.clone() };
        let inner = float_with(buf, area, &title, theme, theme.border().fg(theme.green));
        self.resize(inner.height, inner.width);

        let s = self.parser.screen();
        for row in 0..inner.height {
            for col in 0..inner.width {
                let Some(cell) = s.cell(row, col) else { continue };
                if cell.is_wide_continuation() {
                    continue;
                }
                let mut fg = color(cell.fgcolor());
                let mut bg = color(cell.bgcolor());
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                    if fg == Color::Reset {
                        fg = Color::Black;
                    }
                    if bg == Color::Reset {
                        bg = Color::Gray;
                    }
                }
                let mut style = Style::new().fg(fg).bg(bg);
                for (on, m) in [
                    (cell.bold(), Modifier::BOLD),
                    (cell.dim(), Modifier::DIM),
                    (cell.italic(), Modifier::ITALIC),
                    (cell.underline(), Modifier::UNDERLINED),
                ] {
                    if on {
                        style = style.add_modifier(m);
                    }
                }
                let text = if cell.has_contents() { cell.contents() } else { " " };
                if let Some(c) = buf.cell_mut((inner.x + col, inner.y + row)) {
                    c.set_symbol(text).set_style(style);
                }
            }
        }
        // Draw the cursor ourselves; the hardware cursor stays hidden.
        if !s.hide_cursor() && scrolled == 0 {
            let (r, c) = s.cursor_position();
            if r < inner.height
                && c < inner.width
                && let Some(cell) = buf.cell_mut((inner.x + c, inner.y + r))
            {
                cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Terminal queries the floating terminal answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Query {
    /// DA1, `CSI c`: what kind of terminal this is.
    PrimaryAttributes,
    /// DA2, `CSI > c`.
    SecondaryAttributes,
    /// DSR, `CSI 5 n`: are you OK?
    Status,
    /// DSR, `CSI 6 n`: where's the cursor?
    Cursor,
}

const QUERIES: [(&[u8], Query); 7] = [
    (b"\x1b[c", Query::PrimaryAttributes),
    (b"\x1b[0c", Query::PrimaryAttributes),
    (b"\x1b[>c", Query::SecondaryAttributes),
    (b"\x1b[>0c", Query::SecondaryAttributes),
    (b"\x1b[5n", Query::Status),
    (b"\x1b[6n", Query::Cursor),
    (b"\x1b[?6n", Query::Cursor),
];

/// Finds the queries in `buf`, each with the offset just past it, and where
/// a query that may continue in the next read starts (`buf.len()` if none).
fn scan(buf: &[u8]) -> (Vec<(usize, Query)>, usize) {
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(off) = buf[i..].iter().position(|&b| b == 0x1b) {
        let at = i + off;
        let rest = &buf[at..];
        if let Some((seq, q)) = QUERIES.iter().find(|(seq, _)| rest.starts_with(seq)) {
            found.push((at + seq.len(), *q));
            i = at + seq.len();
        } else if QUERIES.iter().any(|(seq, _)| seq.starts_with(rest)) {
            return (found, at);
        } else {
            i = at + 1;
        }
    }
    (found, buf.len())
}

fn color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Bytes a terminal would send for `key` (xterm conventions).
pub fn encode_key(key: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mut out = Vec::new();
    if alt {
        out.push(0x1b);
    }
    let csi = |s: &str| format!("\x1b[{s}").into_bytes();
    let arrow = |c: char| if app_cursor { format!("\x1bO{c}") } else { format!("\x1b[{c}") }.into_bytes();
    let bytes: Vec<u8> = match key.code {
        KeyCode::Char(c) if ctrl => match c.to_ascii_lowercase() {
            c @ 'a'..='z' => vec![c as u8 - b'a' + 1],
            ' ' | '@' | '2' => vec![0],
            '[' | '3' => vec![0x1b],
            '\\' | '4' => vec![0x1c],
            ']' | '5' => vec![0x1d],
            '^' | '6' => vec![0x1e],
            '_' | '/' | '7' => vec![0x1f],
            _ => return None,
        },
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => csi("Z"),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => arrow('A'),
        KeyCode::Down => arrow('B'),
        KeyCode::Right => arrow('C'),
        KeyCode::Left => arrow('D'),
        KeyCode::Home => arrow('H'),
        KeyCode::End => arrow('F'),
        KeyCode::PageUp => csi("5~"),
        KeyCode::PageDown => csi("6~"),
        KeyCode::Delete => csi("3~"),
        KeyCode::Insert => csi("2~"),
        KeyCode::F(n @ 1..=4) => format!("\x1bO{}", (b'P' + n - 1) as char).into_bytes(),
        KeyCode::F(n) => {
            let code = match n {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                12 => 24,
                _ => return None,
            };
            csi(&format!("{code}~"))
        }
        _ => return None,
    };
    out.extend(bytes);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_queries_even_split_across_reads() {
        let (q, held) = scan(b"hi\x1b[0m\x1b[?u\x1b[0cmore\x1b[6n");
        assert_eq!(q, [(14, Query::PrimaryAttributes), (22, Query::Cursor)]);
        assert_eq!(held, 22);
        let (q, held) = scan(b"text\x1b[");
        assert!(q.is_empty());
        assert_eq!(held, 4, "may become a query");
        let (q, held) = scan(b"text\x1b[31m");
        assert!(q.is_empty());
        assert_eq!(held, 9);
    }

    fn enc(code: KeyCode, m: KeyModifiers) -> Vec<u8> {
        encode_key(&KeyEvent::new(code, m), false).unwrap()
    }

    #[test]
    fn encodes() {
        assert_eq!(enc(KeyCode::Char('a'), KeyModifiers::NONE), b"a");
        assert_eq!(enc(KeyCode::Char('c'), KeyModifiers::CONTROL), [3]);
        assert_eq!(enc(KeyCode::Char('b'), KeyModifiers::ALT), b"\x1bb");
        assert_eq!(enc(KeyCode::Up, KeyModifiers::NONE), b"\x1b[A");
        assert_eq!(encode_key(&KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true).unwrap(), b"\x1bOA");
        assert_eq!(enc(KeyCode::F(5), KeyModifiers::NONE), b"\x1b[15~");
    }
}
