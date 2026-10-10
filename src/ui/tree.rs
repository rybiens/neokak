//! Neo-tree style file explorer sidebar.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{fill, put};
use super::icons::Icons;
use super::theme::Theme;
use crate::git::{GitStatus, Status};

struct Entry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    /// Indent guides ("│ ", "├ ", "└ ") leading up to the entry.
    guides: String,
}

pub enum TreeAction {
    None,
    Open(PathBuf),
    Close,
    Unfocus,
    NewFile(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

pub struct Tree {
    pub root: PathBuf,
    pub width: u16,
    expanded: HashSet<PathBuf>,
    entries: Vec<Entry>,
    selected: usize,
    scroll: usize,
    show_hidden: bool,
    git: GitStatus,
    current: Option<PathBuf>,
}

impl Tree {
    pub fn new(root: PathBuf) -> Self {
        let mut t = Self {
            root,
            width: 32,
            expanded: HashSet::new(),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            show_hidden: false,
            git: GitStatus::default(),
            current: None,
        };
        t.refresh();
        t
    }

    /// Re-reads the directory listing and git status, keeping the selection.
    pub fn refresh(&mut self) {
        let keep = self.selected_path().map(Path::to_path_buf);
        self.git = GitStatus::load(&self.root);
        self.entries.clear();
        let root = self.root.clone();
        self.list(&root, "");
        if let Some(p) = keep {
            self.select(&p);
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    fn list(&mut self, dir: &Path, prefix: &str) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut children: Vec<(String, PathBuf, bool)> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if !self.show_hidden && name.starts_with('.') {
                    return None;
                }
                // Follow symlinks so linked directories expand.
                let is_dir = e.path().is_dir();
                Some((name, e.path(), is_dir))
            })
            .collect();
        children.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        let n = children.len();
        for (i, (name, path, is_dir)) in children.into_iter().enumerate() {
            let last = i + 1 == n;
            let guides = format!("{prefix}{}", if last { "└ " } else { "├ " });
            let expanded = is_dir && self.expanded.contains(&path);
            self.entries.push(Entry { path: path.clone(), name, is_dir, guides });
            if expanded {
                self.list(&path, &format!("{prefix}{}", if last { "  " } else { "│ " }));
            }
        }
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.entries.get(self.selected).map(|e| e.path.as_path())
    }

    fn select(&mut self, path: &Path) -> bool {
        match self.entries.iter().position(|e| e.path == path) {
            Some(i) => {
                self.selected = i;
                true
            }
            None => false,
        }
    }

    /// Marks `file` as the current buffer and expands its parents.
    pub fn reveal(&mut self, file: &Path) {
        self.current = Some(file.to_path_buf());
        if !file.starts_with(&self.root) {
            return;
        }
        let mut changed = false;
        for dir in file.ancestors().skip(1).take_while(|d| d.starts_with(&self.root) && *d != self.root) {
            changed |= self.expanded.insert(dir.to_path_buf());
        }
        if changed || !self.select(file) {
            self.refresh();
            self.select(file);
        }
    }

    /// Directory new files are created in, based on the selection.
    fn target_dir(&self) -> PathBuf {
        match self.entries.get(self.selected) {
            Some(e) if e.is_dir && self.expanded.contains(&e.path) => e.path.clone(),
            Some(e) => e.path.parent().unwrap_or(&self.root).to_path_buf(),
            None => self.root.clone(),
        }
    }

    fn toggle(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
        }
        self.refresh();
    }

    fn activate(&mut self) -> TreeAction {
        let Some(e) = self.entries.get(self.selected) else { return TreeAction::None };
        if e.is_dir {
            let p = e.path.clone();
            self.toggle(&p);
            TreeAction::None
        } else {
            TreeAction::Open(e.path.clone())
        }
    }

    fn collapse_or_parent(&mut self) {
        let Some(e) = self.entries.get(self.selected) else { return };
        if e.is_dir && self.expanded.contains(&e.path) {
            let p = e.path.clone();
            self.toggle(&p);
        } else if let Some(parent) = e.path.parent().map(Path::to_path_buf) {
            self.select(&parent);
        }
    }

    pub fn move_by(&mut self, delta: isize) {
        if !self.entries.is_empty() {
            self.selected = (self.selected as isize + delta).clamp(0, self.entries.len() as isize - 1) as usize;
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> TreeAction {
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return match key.code {
                KeyCode::Char('d') => {
                    self.move_by(10);
                    TreeAction::None
                }
                KeyCode::Char('u') => {
                    self.move_by(-10);
                    TreeAction::None
                }
                _ => TreeAction::None,
            };
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.move_by(isize::MAX / 2),
            KeyCode::Enter | KeyCode::Char('l' | 'o') | KeyCode::Right => return self.activate(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => self.collapse_or_parent(),
            KeyCode::Char('H') => {
                self.show_hidden = !self.show_hidden;
                self.refresh();
            }
            KeyCode::Char('R') => self.refresh(),
            KeyCode::Char('a') => return TreeAction::NewFile(self.target_dir()),
            KeyCode::Char('r') => {
                return self.selected_path().map_or(TreeAction::None, |p| TreeAction::Rename(p.into()));
            }
            KeyCode::Char('d') => {
                return self.selected_path().map_or(TreeAction::None, |p| TreeAction::Delete(p.into()));
            }
            KeyCode::Char('q') => return TreeAction::Close,
            KeyCode::Esc | KeyCode::Tab => return TreeAction::Unfocus,
            _ => {}
        }
        TreeAction::None
    }

    /// Handles a click on screen row `row` inside `area`.
    pub fn click(&mut self, area: Rect, row: u16) -> TreeAction {
        let Some(idx) = (row.checked_sub(area.y + 1)).map(|r| self.scroll + r as usize) else {
            return TreeAction::None;
        };
        if idx >= self.entries.len() {
            return TreeAction::None;
        }
        self.selected = idx;
        self.activate()
    }

    pub fn render(&mut self, buf: &mut Buffer, area: Rect, focused: bool, theme: &Theme, icons: &Icons) {
        fill(buf, area, Style::new().fg(theme.fg).bg(theme.bg));
        if area.width < 4 || area.height < 2 {
            return;
        }
        let sep_x = area.right() - 1;
        for y in area.y..area.bottom() {
            put(buf, sep_x, y, area.right(), "\u{2502}", Style::new().fg(theme.border));
        }
        let max_x = sep_x;

        let root_name =
            self.root.file_name().map_or_else(|| self.root.display().to_string(), |n| n.to_string_lossy().into_owned());
        let header = Style::new().fg(theme.blue).add_modifier(Modifier::BOLD);
        let x = put(buf, area.x + 1, area.y, max_x, icons.folder(true), header);
        put(buf, x, area.y, max_x, &root_name, header);

        let rows = (area.height - 1) as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + rows {
            self.scroll = self.selected + 1 - rows;
        }
        for (row, (i, e)) in self.entries.iter().enumerate().skip(self.scroll).take(rows).enumerate() {
            let y = area.y + 1 + row as u16;
            let selected = i == self.selected;
            let mut base = Style::new().fg(theme.fg).bg(theme.bg);
            if selected {
                base = base.bg(if focused { theme.bg_sel } else { theme.bg_alt });
                buf.set_style(Rect::new(area.x, y, max_x - area.x, 1), base);
            }
            let status = self.git.get(&e.path);
            let name_fg = match status {
                Some(Status::Ignored) => theme.fg_dim,
                Some(Status::Untracked | Status::Added) => theme.green,
                Some(Status::Modified | Status::Renamed) => theme.yellow,
                Some(Status::Deleted | Status::Conflict) => theme.red,
                None if e.is_dir => theme.blue,
                None => theme.fg,
            };
            let mut name_style = base.fg(name_fg);
            if self.current.as_deref() == Some(e.path.as_path()) {
                name_style = name_style.add_modifier(Modifier::BOLD | Modifier::ITALIC);
            }

            let x = put(buf, area.x + 1, y, max_x, &e.guides, base.fg(theme.border));
            let expanded = self.expanded.contains(&e.path);
            let icon = if e.is_dir { icons.folder(expanded) } else { icons.file(&e.name) };
            let icon_style = if e.is_dir { base.fg(theme.blue) } else { base.fg(theme.aqua) };
            let x = put(buf, x, y, max_x, icon, icon_style);
            let letter = status.filter(|s| *s != Status::Ignored).map(|s| s.letter());
            let name_max = if letter.is_some() { max_x.saturating_sub(2) } else { max_x };
            put(buf, x, y, name_max, &e.name, name_style);
            if let Some(l) = letter {
                put(buf, max_x - 2, y, max_x, &l.to_string(), base.fg(name_fg));
            }
        }
    }
}
