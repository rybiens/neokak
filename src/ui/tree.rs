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
    /// Change the explorer's width by this many cells.
    Resize(i32),
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

    /// Marks `file` as the current buffer, leaving the selection alone.
    pub fn mark_current(&mut self, file: &Path) {
        self.current = Some(file.to_path_buf());
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
                KeyCode::Left => TreeAction::Resize(-2),
                KeyCode::Right => TreeAction::Resize(2),
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
            KeyCode::Char('<') => return TreeAction::Resize(-2),
            KeyCode::Char('>') => return TreeAction::Resize(2),
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

        // While focused, the bottom row lists the file keys.
        let hints = focused && area.height >= 6;
        let rows = (area.height - if hints { 2 } else { 1 }) as usize;
        if hints {
            let y = area.bottom() - 1;
            let mut x = area.x + 1;
            for (key, what) in [("a", "new"), ("r", "rename"), ("d", "delete")] {
                x = put(buf, x, y, max_x, key, Style::new().fg(theme.orange).bg(theme.bg));
                x = put(buf, x + 1, y, max_x, what, Style::new().fg(theme.fg_dim).bg(theme.bg)) + 2;
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::rows;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn names(t: &Tree) -> Vec<&str> {
        t.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// root/{docs/, src/main.rs, b.txt, A.md, .hidden}
    fn fixture() -> (tempfile::TempDir, Tree) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        for f in ["src/main.rs", "b.txt", "A.md", ".hidden"] {
            std::fs::write(root.join(f), "").unwrap();
        }
        let tree = Tree::new(root.to_path_buf());
        (dir, tree)
    }

    #[test]
    fn lists_dirs_first_case_insensitively_without_dotfiles() {
        let (_dir, mut t) = fixture();
        assert_eq!(names(&t), ["docs", "src", "A.md", "b.txt"]);
        t.handle_key(&key(KeyCode::Char('H')));
        assert_eq!(names(&t), ["docs", "src", ".hidden", "A.md", "b.txt"]);
    }

    #[test]
    fn expands_collapses_and_opens() {
        let (dir, mut t) = fixture();
        t.handle_key(&key(KeyCode::Char('j'))); // src
        assert!(matches!(t.handle_key(&key(KeyCode::Char('l'))), TreeAction::None));
        assert_eq!(names(&t), ["docs", "src", "main.rs", "A.md", "b.txt"]);
        assert_eq!(t.entries[2].guides, "│ └ ");

        t.handle_key(&key(KeyCode::Char('j'))); // main.rs
        match t.handle_key(&key(KeyCode::Enter)) {
            TreeAction::Open(p) => assert_eq!(p, dir.path().join("src/main.rs")),
            _ => panic!("expected Open"),
        }
        // h on a file jumps to its parent; h again collapses it.
        t.handle_key(&key(KeyCode::Char('h')));
        assert_eq!(t.selected_path(), Some(dir.path().join("src").as_path()));
        t.handle_key(&key(KeyCode::Char('h')));
        assert_eq!(names(&t), ["docs", "src", "A.md", "b.txt"]);
    }

    #[test]
    fn reveal_expands_parents_and_selects() {
        let (dir, mut t) = fixture();
        let file = dir.path().join("src/main.rs");
        t.reveal(&file);
        assert_eq!(t.selected_path(), Some(file.as_path()));
    }

    #[test]
    fn file_operations_target_the_right_paths() {
        let (dir, mut t) = fixture();
        t.reveal(&dir.path().join("src/main.rs"));
        match t.handle_key(&key(KeyCode::Char('a'))) {
            TreeAction::NewFile(d) => assert_eq!(d, dir.path().join("src")),
            _ => panic!("expected NewFile"),
        }
        match t.handle_key(&key(KeyCode::Char('d'))) {
            TreeAction::Delete(p) => assert_eq!(p, dir.path().join("src/main.rs")),
            _ => panic!("expected Delete"),
        }
        assert!(matches!(t.handle_key(&key(KeyCode::Char('q'))), TreeAction::Close));
        assert!(matches!(t.handle_key(&key(KeyCode::Char('>'))), TreeAction::Resize(2)));
        let ctrl_left = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        assert!(matches!(t.handle_key(&ctrl_left), TreeAction::Resize(-2)));
        assert!(matches!(t.handle_key(&key(KeyCode::Esc)), TreeAction::Unfocus));
    }

    #[test]
    fn renders_root_and_entries() {
        let (dir, mut t) = fixture();
        let area = Rect::new(0, 0, 30, 8);
        let mut buf = Buffer::empty(area);
        t.render(&mut buf, area, true, &Theme::default(), &Icons { nerd: false });
        let text = rows(&buf).join("\n");
        let root = dir.path().file_name().unwrap().to_str().unwrap();
        assert!(text.contains(root), "{text}");
        assert!(text.contains("> docs") && text.contains("A.md"), "{text}");
    }

    #[test]
    fn focused_explorer_lists_the_file_keys() {
        let (_dir, mut t) = fixture();
        let area = Rect::new(0, 0, 30, 8);
        let mut buf = Buffer::empty(area);
        t.render(&mut buf, area, true, &Theme::default(), &Icons { nerd: false });
        assert!(rows(&buf)[7].contains("a new  r rename  d delete"), "{:?}", rows(&buf));
        let mut buf = Buffer::empty(area);
        t.render(&mut buf, area, false, &Theme::default(), &Icons { nerd: false });
        assert!(!rows(&buf).join("\n").contains("rename"), "only while focused");
    }
}
