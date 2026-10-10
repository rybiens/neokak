//! Telescope-style fuzzy picker: prompt, results and preview.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Matcher, Nucleo, Utf32Str};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::draw::{float, float_with, put, width};
use super::icons::Icons;
use super::theme::Theme;
use crate::event::{AppEvent, Tx};
use crate::kak::session::quote;

/// What choosing an item does.
#[derive(Clone)]
pub enum Accept {
    /// Item is a path: `edit` it.
    Edit,
    /// Item is an `rg --vimgrep` line: open the file at the match.
    GrepHit,
    /// Item is a buffer name.
    Buffer,
    /// Item is a picker name: open that picker.
    Picker,
    /// Run this Kakoune command with the item as its argument.
    Command(String),
}

pub enum Action {
    None,
    Close,
    /// Evaluate this command in the main client, then close.
    Eval(String),
    OpenPicker(String),
}

enum Source {
    Fuzzy(Nucleo<String>),
    Live { generation: Arc<AtomicU64>, items: Vec<String>, running: bool },
}

pub struct Picker {
    title: String,
    query: String,
    source: Source,
    accept: Accept,
    selected: usize,
    scroll: usize,
    matcher: Matcher,
    preview: Option<(PathBuf, Vec<String>)>,
    home: String,
    cwd: String,
    tx: Tx,
}

const MAX_LIVE_RESULTS: usize = 5000;

impl Picker {
    fn new(title: &str, source: Source, accept: Accept, tx: Tx) -> Self {
        let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
        Self {
            title: title.to_owned(),
            query: String::new(),
            source,
            accept,
            selected: 0,
            scroll: 0,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            preview: None,
            home: std::env::var("HOME").unwrap_or_default(),
            cwd,
            tx,
        }
    }

    fn fuzzy(tx: &Tx) -> Nucleo<String> {
        let tx = tx.clone();
        Nucleo::new(Config::DEFAULT.match_paths(), Arc::new(move || drop(tx.send(AppEvent::Wake))), None, 1)
    }

    fn push(nucleo: &Nucleo<String>, items: impl IntoIterator<Item = String>) {
        let injector = nucleo.injector();
        for item in items {
            injector.push(item, |s, cols| cols[0] = s.as_str().into());
        }
    }

    /// A picker over a fixed list of items.
    pub fn list(title: &str, items: Vec<String>, accept: Accept, tx: Tx) -> Self {
        let nucleo = Self::fuzzy(&tx);
        Self::push(&nucleo, items);
        Self::new(title, Source::Fuzzy(nucleo), accept, tx)
    }

    /// Files under the working directory, honoring .gitignore.
    pub fn files(tx: Tx) -> Self {
        let nucleo = Self::fuzzy(&tx);
        let injector = nucleo.injector();
        thread::spawn(move || {
            for entry in ignore::WalkBuilder::new(".").build().flatten() {
                if entry.file_type().is_some_and(|t| t.is_file()) {
                    let p = entry.path().strip_prefix("./").unwrap_or(entry.path());
                    injector.push(p.display().to_string(), |s, cols| cols[0] = s.as_str().into());
                }
            }
        });
        Self::new("Find Files", Source::Fuzzy(nucleo), Accept::Edit, tx)
    }

    /// Items produced by a shell command (the plugin picker API).
    pub fn command(title: &str, shell: &str, accept: Accept, tx: Tx) -> Self {
        let nucleo = Self::fuzzy(&tx);
        let injector = nucleo.injector();
        let shell = shell.to_owned();
        thread::spawn(move || {
            let Ok(mut child) = Command::new("sh")
                .args(["-c", &shell])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
                if !line.is_empty() {
                    injector.push(line, |s, cols| cols[0] = s.as_str().into());
                }
            }
            let _ = child.wait();
        });
        Self::new(title, Source::Fuzzy(nucleo), accept, tx)
    }

    /// Live grep: ripgrep reruns on every query change.
    pub fn grep(tx: Tx) -> Self {
        let source = Source::Live { generation: Arc::new(AtomicU64::new(0)), items: Vec::new(), running: false };
        Self::new("Live Grep", source, Accept::GrepHit, tx)
    }

    /// Pulls new matches from the background matcher.
    pub fn tick(&mut self) {
        if let Source::Fuzzy(n) = &mut self.source {
            n.tick(10);
        }
    }

    pub fn grep_results(&mut self, generation: u64, lines: Vec<String>, done: bool) {
        if let Source::Live { generation: g, items, running } = &mut self.source
            && g.load(Ordering::SeqCst) == generation
        {
            let room = MAX_LIVE_RESULTS.saturating_sub(items.len());
            items.extend(lines.into_iter().take(room));
            *running = !done;
        }
    }

    fn count(&self) -> (usize, usize) {
        match &self.source {
            Source::Fuzzy(n) => {
                let s = n.snapshot();
                (s.matched_item_count() as usize, s.item_count() as usize)
            }
            Source::Live { items, .. } => (items.len(), items.len()),
        }
    }

    fn item(&self, i: usize) -> Option<String> {
        match &self.source {
            Source::Fuzzy(n) => n.snapshot().get_matched_item(i as u32).map(|it| it.data.clone()),
            Source::Live { items, .. } => items.get(i).cloned(),
        }
    }

    fn query_changed(&mut self, appended: bool) {
        self.selected = 0;
        self.scroll = 0;
        match &mut self.source {
            Source::Fuzzy(n) => {
                n.pattern.reparse(0, &self.query, CaseMatching::Smart, Normalization::Smart, appended);
            }
            Source::Live { generation, items, running } => {
                let generation_id = generation.fetch_add(1, Ordering::SeqCst) + 1;
                items.clear();
                *running = !self.query.is_empty();
                if *running {
                    spawn_rg(self.query.clone(), generation_id, generation.clone(), self.tx.clone());
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let (matched, _) = self.count();
        match key.code {
            KeyCode::Esc => return Action::Close,
            KeyCode::Char('c') if ctrl => return Action::Close,
            KeyCode::Enter => return self.accept(),
            KeyCode::Up | KeyCode::BackTab => self.move_by(-1),
            KeyCode::Char('p' | 'k') if ctrl => self.move_by(-1),
            KeyCode::Down | KeyCode::Tab => self.move_by(1),
            KeyCode::Char('n' | 'j') if ctrl => self.move_by(1),
            KeyCode::PageUp => self.move_by(-10),
            KeyCode::PageDown => self.move_by(10),
            KeyCode::Home if ctrl => self.selected = 0,
            KeyCode::End if ctrl => self.selected = matched.saturating_sub(1),
            KeyCode::Char('u') if ctrl => {
                self.query.clear();
                self.query_changed(false);
            }
            KeyCode::Char('w') if ctrl => {
                let trimmed = self.query.trim_end();
                let cut = trimmed.rfind(|c: char| c.is_whitespace() || c == '/').map_or(0, |i| i + 1);
                self.query.truncate(cut);
                self.query_changed(false);
            }
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    self.query_changed(false);
                }
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.query_changed(true);
            }
            _ => {}
        }
        Action::None
    }

    pub fn paste(&mut self, text: &str) {
        self.query.push_str(text.lines().next().unwrap_or(""));
        self.query_changed(true);
    }

    pub fn move_by(&mut self, delta: isize) {
        let (matched, _) = self.count();
        if matched == 0 {
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, matched as isize - 1) as usize;
    }

    fn accept(&self) -> Action {
        let Some(item) = self.item(self.selected) else { return Action::None };
        match &self.accept {
            Accept::Edit => Action::Eval(format!("edit -- {}", quote(&item))),
            Accept::Buffer => Action::Eval(format!("buffer -- {}", quote(&item))),
            Accept::Picker => Action::OpenPicker(item),
            Accept::Command(cmd) => Action::Eval(format!("{cmd} {}", quote(&item))),
            Accept::GrepHit => match parse_grep(&item) {
                Some((path, line, col, _)) => Action::Eval(format!("edit -- {} {line} {col}", quote(path))),
                None => Action::None,
            },
        }
    }

    /// Display form of an item: paths shortened relative to cwd or home.
    fn display<'a>(&self, item: &'a str) -> std::borrow::Cow<'a, str> {
        if !matches!(self.accept, Accept::Edit) {
            return item.into();
        }
        if let Some(rest) = item.strip_prefix(&self.cwd).and_then(|r| r.strip_prefix('/')) {
            return rest.into();
        }
        if !self.home.is_empty()
            && let Some(rest) = item.strip_prefix(&self.home)
        {
            return format!("~{rest}").into();
        }
        item.into()
    }

    /// The file to preview for the selected item, and the line to center.
    fn preview_target(&self) -> Option<(PathBuf, usize)> {
        let item = self.item(self.selected)?;
        let (path, line) = match self.accept {
            Accept::Edit | Accept::Buffer => (item, 1),
            Accept::GrepHit => {
                let (p, l, _, _) = parse_grep(&item)?;
                (p.to_owned(), l)
            }
            _ => return None,
        };
        let path = PathBuf::from(path);
        path.is_file().then_some((path, line))
    }

    pub fn render(&mut self, buf: &mut Buffer, screen: Rect, theme: &Theme, icons: &Icons) {
        self.tick();
        let w = (screen.width * 9 / 10).min(160);
        let h = (screen.height * 8 / 10).max(8.min(screen.height));
        let outer = Rect::new(screen.x + (screen.width - w) / 2, screen.y + (screen.height - h) / 2, w, h);
        let show_preview = w >= 90 && matches!(self.accept, Accept::Edit | Accept::Buffer | Accept::GrepHit);
        let list_w = if show_preview { w / 2 } else { w };
        let prompt_area = Rect::new(outer.x, outer.y, list_w, 3);
        let results_area = Rect::new(outer.x, outer.y + 3, list_w, h.saturating_sub(3));

        // Prompt
        let accent = theme.border().fg(theme.blue);
        let inner = float_with(buf, prompt_area, &self.title, theme, accent);
        let (matched, total) = self.count();
        let running = matches!(self.source, Source::Live { running: true, .. });
        let counter = format!("{matched}/{total}{} ", if running { "…" } else { "" });
        let counter_x = inner.right().saturating_sub(width(&counter) as u16);
        let x = put(buf, inner.x + 1, inner.y, counter_x, icons.prompt(), theme.float().fg(theme.orange));
        let x = put(buf, x, inner.y, counter_x, &self.query, theme.float());
        put(buf, x, inner.y, counter_x, "\u{258f}", theme.float().fg(theme.fg));
        put(buf, counter_x, inner.y, inner.right(), &counter, theme.float().fg(theme.fg_dim));

        // Results
        let inner = float(buf, results_area, "Results", theme);
        let rows = inner.height as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if rows > 0 && self.selected >= self.scroll + rows {
            self.scroll = self.selected + 1 - rows;
        }
        let pattern = match &self.source {
            Source::Fuzzy(n) => Some(n.snapshot().pattern().column_pattern(0).clone()),
            _ => None,
        };
        let mut indices = Vec::new();
        let mut utf32 = Vec::new();
        for row in 0..rows {
            let i = self.scroll + row;
            let Some(item) = self.item(i) else { break };
            let y = inner.y + row as u16;
            let selected = i == self.selected;
            let base = if selected { theme.float().bg(theme.bg_sel) } else { theme.float() };
            buf.set_style(Rect::new(inner.x, y, inner.width, 1), base);
            let caret = if selected { "\u{258c}" } else { " " };
            let mut x = put(buf, inner.x, y, inner.right(), caret, base.fg(theme.blue));
            if matches!(self.accept, Accept::Edit | Accept::Buffer | Accept::GrepHit) {
                let path = match self.accept {
                    Accept::GrepHit => parse_grep(&item).map_or("", |g| g.0),
                    _ => &item,
                };
                x = put(buf, x, y, inner.right(), icons.file(path), base.fg(theme.blue));
            }

            // Highlight matched characters.
            indices.clear();
            if let Some(p) = &pattern {
                let hay = Utf32Str::new(&item, &mut utf32);
                p.indices(hay, &mut self.matcher, &mut indices);
                indices.sort_unstable();
                indices.dedup();
            }
            let shown = self.display(&item);
            let offset = item.chars().count() as isize - shown.chars().count() as isize;
            let hl = base.fg(theme.orange).add_modifier(Modifier::BOLD);
            let mut matches = indices.iter().map(|&i| i as isize - offset).peekable();
            for (ci, c) in shown.chars().enumerate() {
                while matches.peek().is_some_and(|&m| m < ci as isize) {
                    matches.next();
                }
                let style = if matches.peek() == Some(&(ci as isize)) { hl } else { base };
                x = put(buf, x, y, inner.right(), c.encode_utf8(&mut [0; 4]), style);
            }
        }

        if show_preview {
            let area = Rect::new(outer.x + list_w, outer.y, w - list_w, h);
            self.render_preview(buf, area, theme);
        }
    }

    fn render_preview(&mut self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        let target = self.preview_target();
        let title = target
            .as_ref()
            .map(|(p, _)| self.display(&p.display().to_string()).into_owned())
            .unwrap_or_else(|| "Preview".into());
        let inner = float(buf, area, &title, theme);
        let Some((path, line)) = target else { return };
        if self.preview.as_ref().is_none_or(|(p, _)| *p != path) {
            self.preview = Some((path.clone(), read_preview(&path)));
        }
        let lines = &self.preview.as_ref().unwrap().1;
        let rows = inner.height as usize;
        let first = line.saturating_sub(rows / 2 + 1).min(lines.len().saturating_sub(rows));
        let num_w = (first + rows).to_string().len();
        for (row, text) in lines.iter().skip(first).take(rows).enumerate() {
            let n = first + row + 1;
            let y = inner.y + row as u16;
            let style = if n == line && line > 1 { theme.float().bg(theme.bg_sel) } else { theme.float() };
            buf.set_style(Rect::new(inner.x, y, inner.width, 1), style);
            let x = put(buf, inner.x, y, inner.right(), &format!("{n:>num_w$} "), style.fg(theme.fg_dim));
            put(buf, x + 1, y, inner.right(), text, style);
        }
    }
}

fn read_preview(path: &Path) -> Vec<String> {
    const MAX: u64 = 512 * 1024;
    let Ok(file) = std::fs::File::open(path) else { return vec!["(unreadable)".into()] };
    let mut bytes = Vec::new();
    use std::io::Read;
    let _ = file.take(MAX).read_to_end(&mut bytes);
    if bytes.contains(&0) {
        return vec!["(binary file)".into()];
    }
    String::from_utf8_lossy(&bytes).lines().map(|l| l.replace('\t', "    ")).collect()
}

/// Splits `path:line:col:text`, tolerating colons inside the path.
pub fn parse_grep(s: &str) -> Option<(&str, usize, usize, &str)> {
    let mut search = 0;
    while let Some(off) = s[search..].find(':') {
        let i = search + off;
        let mut parts = s[i + 1..].splitn(3, ':');
        if let (Some(l), Some(c), Some(text)) = (parts.next(), parts.next(), parts.next())
            && let (Ok(l), Ok(c)) = (l.parse(), c.parse())
        {
            return Some((&s[..i], l, c, text));
        }
        search = i + 1;
    }
    None
}

fn spawn_rg(query: String, id: u64, current: Arc<AtomicU64>, tx: Tx) {
    thread::spawn(move || {
        // Debounce: skip the run if the user is still typing.
        thread::sleep(Duration::from_millis(60));
        if current.load(Ordering::SeqCst) != id {
            return;
        }
        let Ok(mut child) = Command::new("rg")
            .args(["--vimgrep", "--smart-case", "--color", "never", "--max-columns", "300", "--", &query])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            let _ = tx.send(AppEvent::GrepResults { generation: id, lines: Vec::new(), done: true });
            return;
        };
        let mut batch = Vec::new();
        let mut last = Instant::now();
        let mut sent = 0;
        for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
            if current.load(Ordering::SeqCst) != id || sent >= MAX_LIVE_RESULTS {
                let _ = child.kill();
                break;
            }
            batch.push(line);
            if batch.len() >= 500 || last.elapsed() > Duration::from_millis(50) {
                sent += batch.len();
                let _ =
                    tx.send(AppEvent::GrepResults { generation: id, lines: std::mem::take(&mut batch), done: false });
                last = Instant::now();
            }
        }
        let _ = child.wait();
        let _ = tx.send(AppEvent::GrepResults { generation: id, lines: batch, done: true });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grep_lines() {
        assert_eq!(parse_grep("src/main.rs:10:5:fn main() {"), Some(("src/main.rs", 10, 5, "fn main() {")));
        assert_eq!(parse_grep("a:b.rs:3:1:x: y"), Some(("a:b.rs", 3, 1, "x: y")));
        assert_eq!(parse_grep("nonsense"), None);
    }
}
