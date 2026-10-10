//! Small single-line input float, used for file tree operations.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::draw::{clamp, float_with, put, scroll_to, width};
use super::theme::Theme;

pub enum Purpose {
    NewFile(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

pub struct Input {
    pub purpose: Purpose,
    title: String,
    pub text: String,
}

pub enum InputResult {
    Pending,
    Cancel,
    Submit,
}

impl Input {
    pub fn new(purpose: Purpose, root: &std::path::Path) -> Self {
        let rel = |p: &PathBuf| p.strip_prefix(root).unwrap_or(p).display().to_string();
        let (title, text) = match &purpose {
            Purpose::NewFile(dir) => {
                let d = rel(dir);
                (
                    format!("New file in {}/ (end with / for a directory)", if d.is_empty() { "." } else { &d }),
                    String::new(),
                )
            }
            Purpose::Rename(p) => ("Rename".to_owned(), rel(p)),
            Purpose::Delete(p) => (format!("Delete {}? (y/N)", rel(p)), String::new()),
        };
        Self { purpose, title, text }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> InputResult {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Purpose::Delete(_) = self.purpose {
            return match key.code {
                KeyCode::Char('y' | 'Y') => InputResult::Submit,
                _ => InputResult::Cancel,
            };
        }
        match key.code {
            KeyCode::Esc => return InputResult::Cancel,
            KeyCode::Char('c') if ctrl => return InputResult::Cancel,
            KeyCode::Enter => {
                return if self.text.trim().is_empty() { InputResult::Cancel } else { InputResult::Submit };
            }
            KeyCode::Backspace => {
                self.text.pop();
            }
            KeyCode::Char('u') if ctrl => self.text.clear(),
            KeyCode::Char(c) if !ctrl => self.text.push(c),
            _ => {}
        }
        InputResult::Pending
    }

    pub fn render(&self, buf: &mut Buffer, screen: Rect, theme: &Theme) {
        let w = (width(&self.title) as u16 + 6).clamp(40, 80).min(screen.width);
        let area = clamp(Rect::new(screen.x + (screen.width - w) / 2, screen.y + screen.height / 4, w, 3), screen);
        let border = theme.border().fg(if matches!(self.purpose, Purpose::Delete(_)) { theme.red } else { theme.blue });
        let inner = float_with(buf, area, &self.title, theme, border);
        if matches!(self.purpose, Purpose::Delete(_)) {
            return;
        }
        let avail = inner.width.saturating_sub(3) as usize;
        let segs = scroll_to(&[(self.text.clone(), theme.float())], width(&self.text), avail);
        let mut x = inner.x + 1;
        for (t, s) in &segs {
            x = put(buf, x, inner.y, inner.right(), t, *s);
        }
        put(buf, x, inner.y, inner.right(), "\u{258f}", theme.float());
    }
}
