//! The settings screen (`<space>o`): every `config.toml` setting and leader
//! binding, edited in place. Changes go to the file (see [`crate::config`]),
//! and the screen follows the file when it changes on disk.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use toml_edit::Value;

use super::draw::{float, put};
use super::theme::Theme;
use crate::config::{self, Config, Kind, SETTINGS};

enum Row {
    Header(String),
    /// Index into [`SETTINGS`], the value shown, and whether the file sets it.
    Setting(usize, String, bool),
    Key {
        spec: String,
        doc: String,
        /// The command, for bindings that run one.
        command: Option<String>,
        custom: bool,
    },
}

/// What the screen asks the app to do.
#[derive(Debug)]
pub enum SettingsAction {
    None,
    Close,
    /// Open config.toml in the editor.
    OpenFile,
    Set(usize, Value),
    Reset(usize),
    /// Bind a leader key sequence to a command (empty: remove the binding).
    SetKey(String, String),
    ResetKey(String),
}

impl PartialEq for SettingsAction {
    fn eq(&self, other: &Self) -> bool {
        // toml_edit values compare by their text.
        format!("{self:?}") == format!("{other:?}")
    }
}

pub struct Settings {
    rows: Vec<Row>,
    selected: usize,
    scroll: usize,
    /// Text being typed for the selected row.
    editing: Option<String>,
    /// A problem to show (bad number, config errors).
    pub message: Option<String>,
    path: String,
}

impl Settings {
    pub fn new(config: &Config, path: &str) -> Self {
        let mut s = Self { rows: Vec::new(), selected: 0, scroll: 0, editing: None, message: None, path: path.into() };
        s.refresh(config);
        s.selected = s.next_selectable(0, 1);
        s
    }

    /// Rebuilds the rows from `config`, keeping the selection.
    pub fn refresh(&mut self, config: &Config) {
        let mut rows = Vec::new();
        let mut section = "";
        for (i, s) in SETTINGS.iter().enumerate() {
            if s.section != section {
                section = s.section;
                rows.push(Row::Header(title_case(section)));
            }
            rows.push(Row::Setting(i, config::display(&config.value(s)), config.get(s).is_some()));
        }
        rows.push(Row::Header("Leader keys".into()));
        let (bindings, _) = config.bindings();
        for b in &bindings {
            let spec = config::spec_of(&bindings, b);
            let custom = config.has_key(&spec);
            rows.push(Row::Key { command: Config::command_of(b), doc: b.doc.clone(), custom, spec });
        }
        // Bindings removed in the file, so they can be restored.
        if let Some(keys) = config.doc.get("keys").and_then(|k| k.as_table_like()) {
            for (spec, v) in keys.iter() {
                if v.as_str() == Some("") {
                    rows.push(Row::Key { spec: spec.into(), doc: "(removed)".into(), command: None, custom: true });
                }
            }
        }
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        if matches!(self.rows.get(self.selected), Some(Row::Header(_))) {
            self.selected = self.next_selectable(self.selected, 1);
        }
    }

    fn next_selectable(&self, from: usize, dir: isize) -> usize {
        let mut i = from as isize;
        while i >= 0 && (i as usize) < self.rows.len() {
            if !matches!(self.rows[i as usize], Row::Header(_)) {
                return i as usize;
            }
            i += dir;
        }
        self.selected
    }

    fn move_by(&mut self, delta: isize) {
        let dir = delta.signum();
        let mut at = self.selected;
        for _ in 0..delta.unsigned_abs() {
            let next = self.next_selectable((at as isize + dir).max(0) as usize, dir);
            if next == at {
                break;
            }
            at = next;
        }
        self.selected = at;
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> SettingsAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some(text) = &mut self.editing {
            match key.code {
                KeyCode::Esc => self.editing = None,
                KeyCode::Char('c') if ctrl => self.editing = None,
                KeyCode::Enter => {
                    let text = self.editing.take().unwrap_or_default();
                    return self.submit(text);
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char('u') if ctrl => text.clear(),
                KeyCode::Char(c) if !ctrl => text.push(c),
                _ => {}
            }
            return SettingsAction::None;
        }
        self.message = None;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => SettingsAction::Close,
            KeyCode::Char('c') if ctrl => SettingsAction::Close,
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_by(1);
                SettingsAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_by(-1);
                SettingsAction::None
            }
            KeyCode::PageDown => {
                self.move_by(10);
                SettingsAction::None
            }
            KeyCode::PageUp => {
                self.move_by(-10);
                SettingsAction::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = self.next_selectable(0, 1);
                SettingsAction::None
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected = self.next_selectable(self.rows.len().saturating_sub(1), -1);
                SettingsAction::None
            }
            KeyCode::Char('e') => SettingsAction::OpenFile,
            KeyCode::Enter | KeyCode::Char(' ') => self.change(1, true),
            KeyCode::Right | KeyCode::Char('l') => self.change(1, false),
            KeyCode::Left | KeyCode::Char('h') => self.change(-1, false),
            KeyCode::Char('r') => match self.rows.get(self.selected) {
                Some(Row::Setting(i, _, true)) => SettingsAction::Reset(*i),
                Some(Row::Key { spec, custom: true, .. }) => SettingsAction::ResetKey(spec.clone()),
                _ => SettingsAction::None,
            },
            KeyCode::Char('d') => match self.rows.get(self.selected) {
                Some(Row::Key { spec, command: Some(_), .. }) => SettingsAction::SetKey(spec.clone(), String::new()),
                _ => SettingsAction::None,
            },
            _ => SettingsAction::None,
        }
    }

    /// Enter (`edit`) or ←/→ on the selected row.
    fn change(&mut self, dir: isize, edit: bool) -> SettingsAction {
        match self.rows.get(self.selected) {
            Some(Row::Setting(i, shown, _)) => {
                let s = &SETTINGS[*i];
                match s.kind {
                    Kind::Bool => SettingsAction::Set(*i, Value::from(shown != "true")),
                    Kind::Choice(choices) => {
                        let at = choices.iter().position(|c| c == shown).unwrap_or(0) as isize;
                        let next = (at + dir).rem_euclid(choices.len() as isize) as usize;
                        SettingsAction::Set(*i, Value::from(choices[next]))
                    }
                    Kind::Int | Kind::Str if edit => {
                        let text = if shown == "(empty)" { String::new() } else { shown.clone() };
                        self.editing = Some(text);
                        SettingsAction::None
                    }
                    Kind::List if edit => SettingsAction::OpenFile,
                    _ => SettingsAction::None,
                }
            }
            Some(Row::Key { command, .. }) if edit => {
                if command.is_none()
                    && !matches!(self.rows.get(self.selected), Some(Row::Key { doc, .. }) if doc == "(removed)")
                {
                    self.message =
                        Some("This binding types keys rather than run a command; edit it in the file".into());
                    return SettingsAction::None;
                }
                self.editing = Some(command.clone().unwrap_or_default());
                SettingsAction::None
            }
            _ => SettingsAction::None,
        }
    }

    fn submit(&mut self, text: String) -> SettingsAction {
        match self.rows.get(self.selected) {
            Some(Row::Setting(i, _, _)) => match SETTINGS[*i].kind {
                Kind::Int => match text.trim().parse::<i64>() {
                    Ok(n) => SettingsAction::Set(*i, Value::from(n)),
                    Err(_) => {
                        self.message = Some(format!("{text:?} isn't a number"));
                        SettingsAction::None
                    }
                },
                _ => SettingsAction::Set(*i, Value::from(text)),
            },
            Some(Row::Key { spec, .. }) => SettingsAction::SetKey(spec.clone(), text.trim().to_owned()),
            _ => SettingsAction::None,
        }
    }

    pub fn render(&mut self, buf: &mut Buffer, screen: Rect, theme: &Theme) {
        let w = (screen.width * 4 / 5).clamp(40.min(screen.width), 110);
        let h = (screen.height * 4 / 5).clamp(10.min(screen.height), 40);
        let area = Rect::new(screen.x + (screen.width - w) / 2, screen.y + (screen.height - h) / 2, w, h);
        let inner = float(buf, area, &format!("Settings  {}", self.path), theme);
        if inner.height < 4 {
            return;
        }
        // Rows, then a blank line, the selected row's description, and hints.
        let list_h = inner.height.saturating_sub(3) as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected.saturating_sub(1);
        } else if self.selected >= self.scroll + list_h {
            self.scroll = self.selected + 1 - list_h;
        }
        let name_w = 26u16.min(inner.width / 2);
        let (right, base) = (inner.right(), theme.float());
        for (row_i, row) in self.rows.iter().enumerate().skip(self.scroll).take(list_h) {
            let y = inner.y + (row_i - self.scroll) as u16;
            let selected = row_i == self.selected;
            let style = if selected { base.bg(theme.bg_sel) } else { base };
            if selected {
                put(buf, inner.x, y, right, &" ".repeat(inner.width as usize), style);
            }
            let x = inner.x + 1;
            match row {
                Row::Header(title) => {
                    put(buf, x, y, right, title, base.fg(theme.blue).add_modifier(Modifier::BOLD));
                }
                Row::Setting(i, shown, set) => {
                    let s = &SETTINGS[*i];
                    put(buf, x, y, right, if *set { "●" } else { " " }, style.fg(theme.yellow));
                    put(buf, x + 2, y, right, s.key, style.fg(theme.fg));
                    let vx = x + name_w;
                    match (&self.editing, selected) {
                        (Some(text), true) => {
                            let end = put(buf, vx, y, right, text, style.fg(theme.fg));
                            put(buf, end, y, right, "\u{258f}", style.fg(theme.blue));
                        }
                        _ => {
                            let color = match s.kind {
                                Kind::Bool if shown == "true" => theme.green,
                                Kind::Bool => theme.red,
                                Kind::Choice(_) => theme.purple,
                                _ => theme.aqua,
                            };
                            let shown = if matches!(s.kind, Kind::Choice(_)) {
                                format!("‹ {shown} ›")
                            } else {
                                shown.clone()
                            };
                            put(buf, vx, y, right, &shown, style.fg(color));
                        }
                    }
                }
                Row::Key { spec, doc, command, custom } => {
                    put(buf, x, y, right, if *custom { "●" } else { " " }, style.fg(theme.yellow));
                    let end = put(buf, x + 2, y, right, "<space>", style.fg(theme.fg_dim));
                    put(buf, end, y, right, spec, style.fg(theme.orange));
                    let vx = x + name_w;
                    match (&self.editing, selected) {
                        (Some(text), true) => {
                            let end = put(buf, vx, y, right, text, style.fg(theme.fg));
                            put(buf, end, y, right, "\u{258f}", style.fg(theme.blue));
                        }
                        _ => {
                            let end = put(buf, vx, y, right, doc, style.fg(theme.fg));
                            if let Some(cmd) = command.as_ref().filter(|c| *c != doc) {
                                put(buf, end + 2, y, right, cmd, style.fg(theme.fg_dim));
                            }
                        }
                    }
                }
            }
        }
        let help_y = inner.bottom() - 2;
        let about = match (&self.message, self.rows.get(self.selected)) {
            (Some(m), _) => (m.clone(), base.fg(theme.red)),
            (None, Some(Row::Setting(i, ..))) => {
                let s = &SETTINGS[*i];
                (
                    format!("{}  ({} = {})", s.doc, s.path(), s.default.lines().next().unwrap_or_default()),
                    base.fg(theme.fg_dim),
                )
            }
            (None, Some(Row::Key { .. })) => {
                ("Enter a Kakoune command; an empty one removes the binding".into(), base.fg(theme.fg_dim))
            }
            _ => (String::new(), base),
        };
        put(buf, inner.x + 1, help_y, right, &about.0, about.1);
        let hints = if self.editing.is_some() {
            "⏎ save  esc cancel"
        } else {
            "⏎ change  ←→ cycle  r reset  d unbind  e edit file  esc close"
        };
        put(buf, inner.x + 1, help_y + 1, right, hints, Style::default().fg(theme.fg_dim).bg(theme.bg_float));
    }
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match (s, c.next()) {
        ("lsp", _) => "LSP".into(),
        ("env", _) => "Project environments".into(),
        ("treesitter", _) => "Tree-sitter".into(),
        (_, Some(f)) => f.to_uppercase().chain(c).collect(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn select(s: &mut Settings, path: &str) {
        let i = SETTINGS.iter().position(|x| x.path() == path).unwrap();
        s.selected = s.rows.iter().position(|r| matches!(r, Row::Setting(j, ..) if *j == i)).unwrap();
    }

    #[test]
    fn toggles_cycles_and_edits() {
        let config = Config::parse("[git]\nsigns = false\n").unwrap();
        let mut s = Settings::new(&config, "config.toml");
        assert!(matches!(s.rows[s.selected], Row::Setting(..)), "starts on a setting, not a header");

        select(&mut s, "git.signs");
        let i = SETTINGS.iter().position(|x| x.path() == "git.signs").unwrap();
        assert_eq!(s.handle_key(&key(KeyCode::Enter)), SettingsAction::Set(i, Value::from(true)));
        assert_eq!(s.handle_key(&key(KeyCode::Char('r'))), SettingsAction::Reset(i));

        select(&mut s, "theme.name");
        let i = SETTINGS.iter().position(|x| x.path() == "theme.name").unwrap();
        assert_eq!(s.handle_key(&key(KeyCode::Right)), SettingsAction::Set(i, Value::from("gruvbox-light")));
        assert_eq!(s.handle_key(&key(KeyCode::Left)), SettingsAction::Set(i, Value::from("kakoune")));
        assert_eq!(s.handle_key(&key(KeyCode::Char('r'))), SettingsAction::None, "not set in the file");

        select(&mut s, "terminal.shell");
        let i = SETTINGS.iter().position(|x| x.path() == "terminal.shell").unwrap();
        assert_eq!(s.handle_key(&key(KeyCode::Enter)), SettingsAction::None);
        for c in "fish".chars() {
            s.handle_key(&key(KeyCode::Char(c)));
        }
        assert_eq!(s.handle_key(&key(KeyCode::Enter)), SettingsAction::Set(i, Value::from("fish")));

        select(&mut s, "editor.tabstop");
        s.handle_key(&key(KeyCode::Enter));
        s.handle_key(&key(KeyCode::Char('x')));
        assert_eq!(s.handle_key(&key(KeyCode::Enter)), SettingsAction::None);
        assert!(s.message.is_some(), "not a number");
    }

    #[test]
    fn edits_and_removes_bindings() {
        let config = Config::default();
        let mut s = Settings::new(&config, "config.toml");
        s.selected = s.rows.iter().position(|r| matches!(r, Row::Key { spec, .. } if spec == "t")).unwrap();
        assert_eq!(s.handle_key(&key(KeyCode::Char('d'))), SettingsAction::SetKey("t".into(), String::new()));
        s.handle_key(&key(KeyCode::Enter));
        assert_eq!(s.editing.as_deref(), Some("neokak-msg terminal"));
        s.handle_key(&key(KeyCode::Esc));
        assert!(s.editing.is_none());

        let config = Config::parse("[keys]\nt = \"\"\n").unwrap();
        s.refresh(&config);
        let removed =
            s.rows.iter().position(|r| matches!(r, Row::Key { spec, doc, .. } if spec == "t" && doc == "(removed)"));
        s.selected = removed.expect("removed bindings stay listed");
        assert_eq!(s.handle_key(&key(KeyCode::Char('r'))), SettingsAction::ResetKey("t".into()));
    }
}
