//! Frontend state and the main event loop.

use std::collections::BTreeMap;
use std::io::Stdout;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::{Frame, Terminal};

use crate::event::{AppEvent, TermSlot, Tx};
use crate::kak::client::KakClient;
use crate::kak::rpc::KakEvent;
use crate::kak::session;
use crate::keys;
use crate::pane::{self, Dir, Pane, PaneId, Separator, Split};
use crate::recent::Recent;
use crate::ui::dashboard::{self, DashAction};
use crate::ui::draw::put;
use crate::ui::icons::Icons;
use crate::ui::input::{Input, InputResult, Purpose};
use crate::ui::picker::{Accept, Action, Picker};
use crate::ui::popup::{self, Toast};
use crate::ui::statusline::{self, ModeLine, StatusContext};
use crate::ui::tabline::{self, Hit};
use crate::ui::terminal::Terminal as FloatTerm;
use crate::ui::theme::{THEMES, Theme};
use crate::ui::tree::{Tree, TreeAction};

pub const MAIN_CLIENT: &str = "main";
const TOAST_TIME: Duration = Duration::from_secs(4);

struct PickerDef {
    title: String,
    source: String,
    accept: String,
}

#[derive(Default, Clone, Copy)]
struct Layout {
    tabline: Rect,
    tree: Option<Rect>,
    /// Area shared by the editor panes.
    main: Rect,
    status: Rect,
    messages: Rect,
}

impl Layout {
    fn new(area: Rect, tree_width: Option<u16>) -> Self {
        let h = area.height;
        let row = |y: u16| Rect::new(area.x, area.y + y.min(h.saturating_sub(1)), area.width, 1.min(h));
        let main = Rect::new(area.x, area.y + 1.min(h), area.width, h.saturating_sub(3));
        // Leave the editor at least half the screen.
        let tw = tree_width.map(|w| w.min(main.width / 2));
        let tree = tw.map(|w| Rect { width: w, ..main });
        let tw = tw.unwrap_or(0);
        Self {
            tabline: row(0),
            tree,
            main: Rect { x: main.x + tw, width: main.width - tw, ..main },
            status: row(h.saturating_sub(2)),
            messages: row(h.saturating_sub(1)),
        }
    }
}

/// The line `git blame` was asked about, and its result once known.
struct Blame {
    client: String,
    file: String,
    line: usize,
    text: Option<String>,
}

/// A mouse drag in progress that resizes part of the layout.
enum Dragging {
    Separator(Separator),
    TreeEdge,
}

/// Explorer width limits (the layout also caps it at half the screen).
const TREE_MIN_WIDTH: u16 = 12;
/// Cells added or removed per resize step.
const RESIZE_STEP: i32 = 2;

/// How to start more clients of the session for splits.
pub struct SessionInfo {
    pub name: String,
    pub log: PathBuf,
}

pub struct App {
    tx: Tx,
    theme: Theme,
    icons: Icons,
    screen: Rect,
    layout: Layout,
    session: SessionInfo,

    panes: Vec<Pane>,
    split: Split,
    focus: PaneId,
    next_id: PaneId,
    pane_rects: Vec<(PaneId, Rect)>,
    separators: Vec<Separator>,
    dragging: Option<Dragging>,

    buffers: Vec<String>,
    current_file: Option<PathBuf>,
    branch: Option<String>,
    recent: Recent,
    pickers: BTreeMap<String, PickerDef>,
    picker: Option<Picker>,
    toasts: Vec<Toast>,
    tab_hits: Vec<Hit>,
    tree: Option<Tree>,
    tree_focused: bool,
    input: Option<Input>,
    term: Option<FloatTerm>,
    term_visible: bool,
    /// A program running in a float until it exits (lazygit, ...).
    tool: Option<FloatTerm>,
    /// The theme options from kak, kept to rebuild the theme when the
    /// colorscheme (seen through the modeline probe) changes.
    theme_settings: (String, bool, Vec<String>),
    face_probe: Vec<(char, crate::kak::rpc::Face)>,
    /// Inline blame for the focused pane's cursor line.
    blame: Option<Blame>,
    dashboard: bool,

    dirty: bool,
    quit: bool,
}

impl App {
    pub fn new(kak: KakClient, session: SessionInfo, tx: Tx, icons: Icons, dashboard: bool) -> Self {
        Self {
            tx,
            theme: Theme::default(),
            icons,
            screen: Rect::default(),
            layout: Layout::default(),
            session,
            panes: vec![Pane::new(0, MAIN_CLIENT.to_owned(), kak)],
            split: Split::Leaf(0),
            focus: 0,
            next_id: 1,
            pane_rects: Vec::new(),
            separators: Vec::new(),
            dragging: None,
            buffers: Vec::new(),
            current_file: None,
            branch: git_branch(),
            recent: Recent::load(),
            pickers: BTreeMap::new(),
            picker: None,
            toasts: Vec::new(),
            tab_hits: Vec::new(),
            tree: None,
            tree_focused: false,
            input: None,
            term: None,
            term_visible: false,
            tool: None,
            blame: None,
            theme_settings: ("gruvbox".into(), true, Vec::new()),
            face_probe: Vec::new(),
            dashboard,
            dirty: true,
            quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>, rx: Receiver<AppEvent>) -> Result<()> {
        let size = terminal.size()?;
        self.relayout(Rect::new(0, 0, size.width, size.height));
        while !self.quit {
            let ev = if self.toasts.is_empty() {
                Some(rx.recv()?)
            } else {
                match rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(ev) => Some(ev),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            };
            if let Some(ev) = ev {
                self.handle(ev);
            }
            while let Ok(ev) = rx.try_recv() {
                self.handle(ev);
            }
            self.expire_toasts();
            if self.dirty && !self.quit {
                terminal.draw(|f| self.render(f))?;
                self.dirty = false;
            }
        }
        match self.panes.first_mut() {
            Some(p) => p.kak.kill_session(),
            None => session::send_command(&self.session.name, "kill!"),
        }
        Ok(())
    }

    fn pane(&self) -> &Pane {
        self.panes.iter().find(|p| p.id == self.focus).unwrap_or(&self.panes[0])
    }

    fn pane_mut(&mut self) -> &mut Pane {
        let focus = self.focus;
        let i = self.panes.iter().position(|p| p.id == focus).unwrap_or(0);
        &mut self.panes[i]
    }

    fn current(&self) -> &str {
        &self.pane().current
    }

    fn relayout(&mut self, area: Rect) {
        self.screen = area;
        self.layout = Layout::new(area, self.tree.as_ref().map(|t| t.width));
        self.pane_rects.clear();
        self.separators.clear();
        self.split.layout(self.layout.main, &mut self.pane_rects, &mut self.separators);
        for (id, rect) in &self.pane_rects {
            if let Some(p) = self.panes.iter_mut().find(|p| p.id == *id) {
                p.resize(*rect);
            }
        }
        self.dirty = true;
    }

    fn pane_rect(&self, id: PaneId) -> Rect {
        self.pane_rects.iter().find(|(i, _)| *i == id).map_or(self.layout.main, |(_, r)| *r)
    }

    fn handle(&mut self, ev: AppEvent) {
        self.dirty = true;
        match ev {
            AppEvent::Kak(id, ev) => {
                if let KakEvent::DrawStatus { mode_line, .. } = &ev {
                    let probe = statusline::face_probe(mode_line);
                    if !probe.is_empty() && probe != self.face_probe {
                        self.face_probe = probe;
                        if self.theme_settings.0 == "kakoune" {
                            self.rebuild_theme();
                        }
                    }
                }
                if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                    p.handle_kak(ev);
                }
            }
            AppEvent::KakExited(id) => {
                crate::debug!("kak client for pane {id} exited");
                self.remove_pane(id)
            }
            AppEvent::Quit => {
                crate::debug!("quit requested");
                self.quit = true
            }
            AppEvent::Term(ev) => self.handle_term(ev),
            AppEvent::Ipc(args) => self.handle_ipc(args),
            AppEvent::Wake => {}
            AppEvent::PtyOutput(slot, bytes) => {
                let term = match slot {
                    TermSlot::Shell => &mut self.term,
                    TermSlot::Tool => &mut self.tool,
                };
                if let Some(t) = term {
                    t.process(&bytes);
                }
                // Hidden terminal output doesn't need a frame.
                self.dirty = slot == TermSlot::Tool || self.term_visible;
            }
            AppEvent::PtyExited(TermSlot::Tool) => {
                self.tool = None;
                // The tool may have changed files or git state.
                if let Some(tree) = &mut self.tree {
                    tree.refresh();
                }
                self.eval("try %{ git update-diff }");
            }
            AppEvent::Blame { client, file, line, text } => {
                if let Some(b) = &mut self.blame
                    && (b.client.as_str(), b.file.as_str(), b.line) == (client.as_str(), file.as_str(), line)
                {
                    b.text = text;
                }
            }
            AppEvent::PtyExited(TermSlot::Shell) => {
                self.term = None;
                self.term_visible = false;
            }
            AppEvent::GrepResults { generation, lines, done } => {
                if let Some(p) = &mut self.picker {
                    p.grep_results(generation, lines, done);
                }
            }
        }
    }

    fn remove_pane(&mut self, id: PaneId) {
        crate::debug!("remove pane {id}");
        let Some(i) = self.panes.iter().position(|p| p.id == id) else { return };
        let mut pane = self.panes.remove(i);
        if pane.client == MAIN_CLIENT && !self.panes.is_empty() {
            // That process hosts the session server; leave it running.
            pane.kak.detach();
        }
        drop(pane);
        if self.panes.is_empty() {
            self.quit = true;
            return;
        }
        self.split.remove(id);
        if self.focus == id {
            self.focus = self.split.ids()[0];
        }
        self.relayout(self.screen);
    }

    fn split_pane(&mut self, dir: Dir) {
        let id = self.next_id;
        self.next_id += 1;
        let client = format!("split{id}");
        let mut cmd = Command::new("kak");
        cmd.args(["-c", &self.session.name, "-ui", "json", "-e"])
            .arg(format!("rename-client {client}; try %{{ buffer -- {} }}", session::quote(self.current())));
        match KakClient::spawn(cmd, self.session.name.clone(), &self.session.log, id, self.tx.clone()) {
            Ok(kak) => {
                let mut pane = Pane::new(id, client, kak);
                pane.current = self.current().to_owned();
                self.panes.push(pane);
                self.split.split(self.focus, dir, id);
                self.focus = id;
                self.relayout(self.screen);
            }
            Err(e) => self.notify("error", &format!("split: {e}")),
        }
    }

    fn focus_pane(&mut self, id: PaneId) {
        if self.focus != id {
            self.focus = id;
            // Re-sync the file tree and recent list to the newly focused pane.
            self.eval("neokak-sync-current");
        }
        self.tree_focused = false;
    }

    fn window_command(&mut self, cmd: &str) {
        let dir = |c: &str| match c {
            "h" => Some((-1, 0)),
            "j" => Some((0, 1)),
            "k" => Some((0, -1)),
            "l" => Some((1, 0)),
            _ => None,
        };
        match cmd {
            "vsplit" => self.split_pane(Dir::Vertical),
            "split" => self.split_pane(Dir::Horizontal),
            "close" => self.eval("quit"),
            "only" => {
                let others: Vec<String> =
                    self.panes.iter().filter(|p| p.id != self.focus).map(|p| p.client.clone()).collect();
                for c in others {
                    session::eval_in_client(&self.session.name, &c, "quit");
                }
            }
            "equalize" => {
                self.split.equalize();
                self.relayout(self.screen);
            }
            c if c.starts_with("resize-") => self.resize(&c["resize-".len()..]),
            "next" => {
                let ids = self.split.ids();
                let i = ids.iter().position(|&i| i == self.focus).unwrap_or(0);
                self.focus_pane(ids[(i + 1) % ids.len()]);
            }
            c => match dir(c) {
                // Moving left out of the leftmost pane lands in the tree.
                Some((-1, 0))
                    if pane::neighbor(&self.pane_rects, self.focus, -1, 0).is_none() && self.tree.is_some() =>
                {
                    self.tree_focused = true;
                }
                Some((dx, dy)) => {
                    if self.tree_focused && dx == 1 {
                        self.tree_focused = false;
                    } else if let Some(id) = pane::neighbor(&self.pane_rects, self.focus, dx, dy) {
                        self.focus_pane(id);
                    }
                }
                None => self.notify("error", &format!("unknown window command: {c}")),
            },
        }
    }

    /// `resize-left/right/up/down`: like Ctrl+arrows in LazyVim, change the
    /// focused pane's (or the explorer's) width and height.
    fn resize(&mut self, which: &str) {
        let (dir, delta) = match which {
            "right" => (Dir::Vertical, RESIZE_STEP),
            "left" => (Dir::Vertical, -RESIZE_STEP),
            "up" => (Dir::Horizontal, RESIZE_STEP),
            "down" => (Dir::Horizontal, -RESIZE_STEP),
            other => return self.notify("error", &format!("unknown resize direction: {other}")),
        };
        if self.tree_focused {
            if dir == Dir::Vertical {
                self.resize_tree(delta);
            }
            return;
        }
        if self.split.resize(self.layout.main, self.focus, dir, delta) {
            self.relayout(self.screen);
        }
    }

    fn resize_tree(&mut self, delta: i32) {
        if let Some(tree) = &mut self.tree {
            let max = (self.screen.width / 2).max(TREE_MIN_WIDTH);
            tree.width = (tree.width as i32 + delta).clamp(TREE_MIN_WIDTH as i32, max as i32) as u16;
            self.relayout(self.screen);
        }
    }

    fn handle_term(&mut self, ev: Event) {
        match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if let Some(picker) = &mut self.picker {
                    let action = picker.handle_key(&key);
                    self.picker_action(action);
                } else if let Some(input) = &mut self.input {
                    match input.handle_key(&key) {
                        InputResult::Pending => {}
                        InputResult::Cancel => self.input = None,
                        InputResult::Submit => {
                            let input = self.input.take().unwrap();
                            self.file_op(input);
                        }
                    }
                } else if let Some(tool) = &mut self.tool {
                    tool.key(&key);
                } else if is_term_toggle(&key) {
                    self.toggle_terminal();
                } else if let (Some(term), true) = (&mut self.term, self.term_visible) {
                    term.key(&key);
                } else if let Some(action) = self.dashboard_key(&key) {
                    self.dashboard_action(action);
                } else if let (Some(tree), true) = (&mut self.tree, self.tree_focused) {
                    let action = tree.handle_key(&key);
                    self.tree_action(action);
                } else if let Some(k) = keys::to_kak(&key) {
                    self.pane_mut().kak.keys(&[k]);
                }
            }
            Event::Paste(text) => match (&mut self.picker, &mut self.term) {
                (Some(p), _) => p.paste(&text),
                _ if self.tool.is_some() => self.tool.as_mut().unwrap().paste(&text),
                (None, Some(t)) if self.term_visible => t.paste(&text),
                _ => self.pane_mut().kak.paste(&text),
            },
            Event::Mouse(m) => self.handle_mouse(m),
            Event::Resize(w, h) => self.relayout(Rect::new(0, 0, w, h)),
            _ => {}
        }
    }

    fn handle_mouse(&mut self, m: crossterm::event::MouseEvent) {
        if let Some(t) = &mut self.tool {
            match m.kind {
                MouseEventKind::ScrollUp => t.scroll(3),
                MouseEventKind::ScrollDown => t.scroll(-3),
                _ => {}
            }
            return;
        }
        if let (Some(t), true) = (&mut self.term, self.term_visible) {
            match m.kind {
                MouseEventKind::ScrollUp => t.scroll(3),
                MouseEventKind::ScrollDown => t.scroll(-3),
                _ => {}
            }
            return;
        }
        if let Some(p) = &mut self.picker {
            match m.kind {
                MouseEventKind::ScrollUp => p.move_by(-1),
                MouseEventKind::ScrollDown => p.move_by(1),
                _ => {}
            }
            return;
        }
        let pos = Position::new(m.column, m.row);
        if self.drag(&m, pos) {
            return;
        }
        if let (Some(area), Some(tree)) = (self.layout.tree, &mut self.tree)
            && area.contains(pos)
        {
            let action = match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.tree_focused = true;
                    tree.click(area, m.row)
                }
                MouseEventKind::ScrollUp => {
                    tree.move_by(-3);
                    TreeAction::None
                }
                MouseEventKind::ScrollDown => {
                    tree.move_by(3);
                    TreeAction::None
                }
                _ => TreeAction::None,
            };
            self.tree_action(action);
            return;
        }
        if self.layout.tabline.contains(pos) {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind
                && let Some(hit) = self.tab_hits.iter().find(|h| (h.start..h.end).contains(&m.column))
            {
                self.eval(&format!("buffer -- {}", session::quote(&hit.buffer)));
            }
            return;
        }
        let Some(&(id, rect)) = self.pane_rects.iter().find(|(_, r)| r.contains(pos)) else { return };
        if let MouseEventKind::Down(_) = m.kind {
            self.focus_pane(id);
        } else if id != self.focus {
            // Drags and scrolls go to the pane under the pointer only if it
            // already has focus.
            if !matches!(m.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
                return;
            }
        }
        let (line, col) = (m.row - rect.y, m.column - rect.x);
        let button = |b: MouseButton| match b {
            MouseButton::Left => "left",
            MouseButton::Right => "right",
            MouseButton::Middle => "middle",
        };
        let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) else { return };
        match m.kind {
            MouseEventKind::Down(b) => pane.kak.mouse_press(button(b), line, col),
            MouseEventKind::Up(b) => pane.kak.mouse_release(button(b), line, col),
            MouseEventKind::Drag(_) => pane.kak.mouse_move(line, col),
            MouseEventKind::ScrollUp => pane.kak.scroll(-3, line, col),
            MouseEventKind::ScrollDown => pane.kak.scroll(3, line, col),
            _ => {}
        }
    }

    /// Starts, continues or ends a drag on a separator or the explorer's
    /// edge. Returns whether the event was used.
    fn drag(&mut self, m: &crossterm::event::MouseEvent, pos: Position) -> bool {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(area) = self.layout.tree
                    && m.column == area.right().saturating_sub(1)
                    && area.contains(pos)
                {
                    self.dragging = Some(Dragging::TreeEdge);
                    return true;
                }
                if let Some(sep) = self.separators.iter().find(|s| s.rect.contains(pos)) {
                    self.dragging = Some(Dragging::Separator(sep.clone()));
                    return true;
                }
                false
            }
            MouseEventKind::Drag(MouseButton::Left) => match &self.dragging {
                Some(Dragging::TreeEdge) => {
                    let x = self.layout.tree.map_or(0, |a| a.x);
                    let width = (m.column + 1).saturating_sub(x) as i32;
                    let current = self.tree.as_ref().map_or(0, |t| t.width) as i32;
                    self.resize_tree(width - current);
                    true
                }
                Some(Dragging::Separator(sep)) => {
                    let at = if sep.dir == Dir::Vertical { m.column } else { m.row };
                    let sep = sep.clone();
                    self.split.drag(self.layout.main, &sep, at);
                    self.relayout(self.screen);
                    true
                }
                None => false,
            },
            MouseEventKind::Up(MouseButton::Left) => self.dragging.take().is_some(),
            _ => false,
        }
    }

    fn handle_ipc(&mut self, args: Vec<String>) {
        crate::debug!("ipc {args:?}");
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or_default();
        match arg(0) {
            "current" => {
                let Some(pane) = self.panes.iter_mut().find(|p| p.client == arg(1)) else { return };
                pane.current = arg(2).to_owned();
                if pane.id != self.focus {
                    return;
                }
                if arg(2) != "*scratch*" {
                    self.dashboard = false;
                }
                self.recent.touch(arg(3));
                self.branch = git_branch();
                let file = PathBuf::from(arg(3));
                if file.is_absolute() {
                    if let Some(tree) = &mut self.tree {
                        tree.reveal(&file);
                    }
                    self.current_file = Some(file);
                }
            }
            "clientclose" => {
                if let Some(id) = self.panes.iter().find(|p| p.client == arg(1)).map(|p| p.id) {
                    self.remove_pane(id);
                }
            }
            "buflist" => self.buffers = args[1..].to_vec(),
            "bufclose" => self.buffers.retain(|b| b != arg(1)),
            "picker" => self.open_picker(arg(1)),
            "picker-define" => {
                let def = PickerDef { title: arg(2).to_owned(), source: arg(3).to_owned(), accept: arg(4).to_owned() };
                self.pickers.insert(arg(1).to_owned(), def);
            }
            "theme" => {
                self.theme_settings =
                    (arg(1).to_owned(), arg(2) != "false", args.get(3..).unwrap_or_default().to_vec());
                for e in self.rebuild_theme() {
                    self.notify("error", &e);
                }
            }
            "lsp-results" => {
                let content = std::fs::read_to_string(arg(4)).unwrap_or_default();
                let _ = std::fs::remove_file(arg(4));
                let title = match arg(5) {
                    "" => crate::lsp::title(arg(1), arg(2)),
                    custom => custom.to_owned(),
                };
                let items = crate::lsp::items(arg(2), Path::new(arg(3)), &content);
                if items.is_empty() {
                    self.notify("info", &format!("{title}: nothing found"));
                } else {
                    self.picker = Some(Picker::list(&title, items, Accept::Location, self.tx.clone()));
                }
            }
            "float" => self.run_floating(&args[1..]),
            "blame" => {
                let (client, file, line) = (arg(1), arg(2), arg(3).parse().unwrap_or(0));
                // Line numbers don't match the committed file while there
                // are unsaved edits.
                if arg(4) == "true" || line == 0 || !Path::new(file).is_file() {
                    self.blame = None;
                    return;
                }
                if self
                    .blame
                    .as_ref()
                    .is_some_and(|b| (b.client.as_str(), b.file.as_str(), b.line) == (client, file, line))
                {
                    return;
                }
                self.blame = Some(Blame { client: client.into(), file: file.into(), line, text: None });
                let (tx, client, file) = (self.tx.clone(), client.to_owned(), file.to_owned());
                std::thread::spawn(move || {
                    let text = crate::git::blame_line(Path::new(&file), line);
                    let _ = tx.send(AppEvent::Blame { client, file, line, text });
                });
            }
            "blame-off" => self.blame = None,
            "tree" => self.toggle_tree(),
            "terminal" => self.toggle_terminal(),
            "window" => self.window_command(arg(1)),
            "written" => {
                if let Some(tree) = &mut self.tree {
                    tree.refresh();
                }
            }
            "notify" => match args.len() {
                0..=1 => {}
                2 => self.notify("info", arg(1)),
                _ => self.notify(arg(1), &args[2..].join(" ")),
            },
            _ => {}
        }
    }

    /// Rebuilds the theme from its settings; returns setting errors.
    fn rebuild_theme(&mut self) -> Vec<String> {
        let (name, transparent, overrides) = &self.theme_settings;
        let (theme, errors) = Theme::from_settings(name, *transparent, overrides, &self.face_probe);
        self.theme = theme;
        errors
    }

    fn notify(&mut self, level: &str, message: &str) {
        self.toasts.push(Toast {
            level: level.to_owned(),
            message: message.to_owned(),
            until: Instant::now() + TOAST_TIME,
        });
    }

    fn expire_toasts(&mut self) {
        let before = self.toasts.len();
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.len() != before {
            self.dirty = true;
        }
    }

    /// Evaluates `cmd` in the focused pane's client.
    fn eval(&self, cmd: &str) {
        session::eval_in_client(&self.session.name, &self.pane().client, cmd);
    }

    fn open_picker(&mut self, name: &str) {
        let tx = self.tx.clone();
        let picker = match name {
            "files" => Picker::files(tx),
            "grep" => Picker::grep(tx),
            "buffers" => {
                let current = self.current().to_owned();
                let mut bufs: Vec<String> = tabline::visible(&self.buffers, &current).into_iter().cloned().collect();
                // Most useful first: everything except the buffer you're in.
                bufs.sort_by_key(|b| *b == current);
                Picker::list("Buffers", bufs, Accept::Buffer, tx)
            }
            "recent" => Picker::list("Recent Files", self.recent.files.clone(), Accept::Edit, tx),
            "git-status" => Picker::command(
                "Changed Files",
                "git ls-files --modified --others --exclude-standard --deduplicate 2>/dev/null",
                Accept::Edit,
                tx,
            ),
            "themes" => {
                let names = THEMES.iter().map(|s| s.to_string()).collect();
                Picker::list("Themes", names, Accept::Command("neokak-theme".into()), tx)
            }
            "pickers" => {
                let mut names: Vec<String> =
                    ["files", "grep", "buffers", "recent", "git-status", "themes"].map(String::from).to_vec();
                names.extend(self.pickers.keys().cloned());
                Picker::list("Pickers", names, Accept::Picker, tx)
            }
            other => match self.pickers.get(other) {
                Some(def) => Picker::command(&def.title, &def.source, Accept::Command(def.accept.clone()), tx),
                None => {
                    self.notify("error", &format!("unknown picker: {other}"));
                    return;
                }
            },
        };
        self.picker = Some(picker);
    }

    /// Runs a program in a floating terminal until it exits.
    fn run_floating(&mut self, command: &[String]) {
        let Some(prog) = command.first().filter(|p| !p.is_empty()) else { return };
        if self.tool.is_some() {
            return self.notify("warn", "a floating program is already running");
        }
        if !on_path(prog) {
            return self.notify("error", &format!("{prog} not found"));
        }
        let inner = FloatTerm::area(self.screen);
        let (rows, cols) = (inner.height.saturating_sub(2).max(1), inner.width.saturating_sub(2).max(1));
        match FloatTerm::spawn(rows, cols, Some(command), TermSlot::Tool, self.tx.clone()) {
            Ok(t) => self.tool = Some(t),
            Err(e) => self.notify("error", &format!("{prog}: {e}")),
        }
    }

    fn toggle_terminal(&mut self) {
        if self.term_visible {
            self.term_visible = false;
            return;
        }
        if self.term.is_none() {
            let inner = FloatTerm::area(self.screen);
            let (rows, cols) = (inner.height.saturating_sub(2).max(1), inner.width.saturating_sub(2).max(1));
            match FloatTerm::spawn(rows, cols, None, TermSlot::Shell, self.tx.clone()) {
                Ok(t) => self.term = Some(t),
                Err(e) => return self.notify("error", &format!("terminal: {e}")),
            }
        }
        self.term_visible = true;
    }

    /// Dashboard buttons, while it's showing; any other key dismisses it.
    fn dashboard_key(&mut self, key: &KeyEvent) -> Option<DashAction> {
        if !self.dashboard {
            return None;
        }
        let action = dashboard::action_for(key, &self.icons);
        if action.is_none() {
            self.dashboard = false;
        }
        action
    }

    fn dashboard_action(&mut self, action: DashAction) {
        match action {
            DashAction::Picker(name) => self.open_picker(name),
            DashAction::Explorer => self.toggle_tree(),
            DashAction::NewFile => {
                self.dashboard = false;
                self.pane_mut().kak.keys(&["i".to_owned()]);
            }
            DashAction::Config => {
                let dir = std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
                    .unwrap_or_default()
                    .join("neokak");
                let _ = std::fs::create_dir_all(&dir);
                self.eval(&format!("edit -- {}", session::quote(&dir.join("init.kak").display().to_string())));
            }
            DashAction::Quit => self.eval("quit"),
        }
    }

    /// `<space>e`: open and focus, focus if open, close if focused.
    fn toggle_tree(&mut self) {
        match (&mut self.tree, self.tree_focused) {
            (None, _) => {
                let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let mut tree = Tree::new(root);
                if let Some(f) = &self.current_file {
                    tree.reveal(f);
                }
                self.tree = Some(tree);
                self.tree_focused = true;
            }
            (Some(_), false) => self.tree_focused = true,
            (Some(_), true) => {
                self.tree = None;
                self.tree_focused = false;
            }
        }
        self.relayout(self.screen);
    }

    fn tree_action(&mut self, action: TreeAction) {
        let root = self.tree.as_ref().map(|t| t.root.clone()).unwrap_or_default();
        match action {
            TreeAction::None => {}
            TreeAction::Open(path) => {
                self.tree_focused = false;
                self.eval(&format!("edit -- {}", session::quote(&path.display().to_string())));
            }
            TreeAction::Close => {
                self.tree = None;
                self.tree_focused = false;
                self.relayout(self.screen);
            }
            TreeAction::Unfocus => self.tree_focused = false,
            TreeAction::Resize(delta) => self.resize_tree(delta),
            TreeAction::NewFile(dir) => self.input = Some(Input::new(Purpose::NewFile(dir), &root)),
            TreeAction::Rename(p) => self.input = Some(Input::new(Purpose::Rename(p), &root)),
            TreeAction::Delete(p) => self.input = Some(Input::new(Purpose::Delete(p), &root)),
        }
    }

    /// Kakoune buffer names that refer to `path` (or to files under it).
    fn buffers_under(&self, path: &Path) -> Vec<String> {
        let cwd = std::env::current_dir().unwrap_or_default();
        self.buffers.iter().filter(|b| !b.starts_with('*') && cwd.join(b).starts_with(path)).cloned().collect()
    }

    fn file_op(&mut self, input: Input) {
        let root = self.tree.as_ref().map(|t| t.root.clone()).unwrap_or_default();
        let q = |p: &Path| session::quote(&p.display().to_string());
        let result = match &input.purpose {
            Purpose::NewFile(dir) => {
                let name = input.text.trim();
                let path = dir.join(name);
                let r = if name.ends_with('/') {
                    std::fs::create_dir_all(&path)
                } else {
                    path.parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|_| std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map(drop))
                };
                if r.is_ok() && !name.ends_with('/') {
                    self.eval(&format!("edit -- {}", q(&path)));
                    self.tree_focused = false;
                }
                r.map(|_| Some(path))
            }
            Purpose::Rename(from) => {
                let to = root.join(input.text.trim());
                let r = to.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::rename(from, &to));
                if r.is_ok() {
                    // Point open buffers at the new location.
                    let cwd = std::env::current_dir().unwrap_or_default();
                    for b in self.buffers_under(from) {
                        let rel = cwd.join(&b).strip_prefix(from).map(Path::to_path_buf).unwrap_or_default();
                        let new = if rel.as_os_str().is_empty() { to.clone() } else { to.join(rel) };
                        self.eval(&format!(
                            "try %{{ evaluate-commands -buffer {} %{{ rename-buffer -file {} }} }}",
                            session::quote(&b),
                            q(&new)
                        ));
                    }
                    // Kakoune has no hook for buffer renames.
                    self.eval("neokak-sync-buflist; neokak-sync-current");
                }
                r.map(|_| Some(to))
            }
            Purpose::Delete(path) => {
                let r = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
                if r.is_ok() {
                    for b in self.buffers_under(path) {
                        self.eval(&format!("try %{{ delete-buffer! {} }}", session::quote(&b)));
                    }
                }
                r.map(|_| None)
            }
        };
        match result {
            Ok(select) => {
                if let Some(tree) = &mut self.tree {
                    tree.refresh();
                    if let Some(p) = select {
                        tree.reveal(&p);
                    }
                }
            }
            Err(e) => self.notify("error", &e.to_string()),
        }
    }

    fn picker_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Close => self.picker = None,
            Action::Eval(cmd) => {
                self.picker = None;
                self.eval(&cmd);
            }
            Action::OpenPicker(name) => self.open_picker(&name),
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let screen = f.area();
        let layout = self.layout;
        let focus_rect = self.pane_rect(self.focus);
        let buf = f.buffer_mut();
        let (theme, icons) = (&self.theme, &self.icons);

        let focused = self.panes.iter().find(|p| p.id == self.focus).unwrap_or(&self.panes[0]);
        let modified = ModeLine::parse(&focused.status.mode_line).is_some_and(|m| m.modified);
        let mut tabs_area = layout.tabline;
        if let (Some(area), Some(tree)) = (layout.tree, &mut self.tree) {
            tree.render(buf, area, self.tree_focused, theme, icons);
            // Like bufferline's neo-tree offset: label the explorer column.
            let label = Rect { width: area.width, ..layout.tabline };
            tabline::render_offset(buf, label, "File Explorer", theme);
            tabs_area = Rect { x: label.right(), width: layout.tabline.width - label.width, ..layout.tabline };
        }
        self.tab_hits = tabline::render(buf, tabs_area, &self.buffers, &focused.current, modified, theme, icons);

        for (id, rect) in &self.pane_rects {
            if let Some(p) = self.panes.iter().find(|p| p.id == *id) {
                p.editor.render(buf, *rect);
            }
        }
        let multi = self.pane_rects.len() > 1;
        for Separator { dir, rect, .. } in &self.separators {
            let (glyph, cells) = match dir {
                Dir::Vertical => ("\u{2502}", rect.height),
                Dir::Horizontal => ("\u{2500}", rect.width),
            };
            for i in 0..cells {
                let (x, y) = match dir {
                    Dir::Vertical => (rect.x, rect.y + i),
                    Dir::Horizontal => (rect.x + i, rect.y),
                };
                // Highlight the separator cells bordering the focused pane.
                let near = match dir {
                    Dir::Vertical => {
                        (x + 1 == focus_rect.x || x == focus_rect.right())
                            && focus_rect.y <= y
                            && y < focus_rect.bottom()
                    }
                    Dir::Horizontal => {
                        (y + 1 == focus_rect.y || y == focus_rect.bottom())
                            && focus_rect.x <= x
                            && x < focus_rect.right()
                    }
                };
                let color = if multi && near && !self.tree_focused { theme.blue } else { theme.border };
                put(buf, x, y, x + 1, glyph, Style::new().fg(color));
            }
        }
        if self.dashboard {
            dashboard::render(buf, focus_rect, theme, icons);
        }
        // Inline blame after the cursor line (gitsigns' current_line_blame).
        if let Some(Blame { client, line, text: Some(text), .. }) = &self.blame
            && *client == focused.client
            && ModeLine::parse(&focused.status.mode_line).is_some_and(|m| m.line == line.to_string() && !m.modified)
        {
            focused.editor.render_after_cursor_line(buf, focus_rect, text, theme.float().bg(theme.bg).fg(theme.fg_dim));
        }

        let status = &focused.status;
        let ctx = StatusContext { mode_line: &status.mode_line, branch: self.branch.as_deref() };
        statusline::render(buf, layout.status, &ctx, theme, icons);
        let prompt = status.is_prompt();
        let messages = if prompt { &Vec::new() } else { &status.content };
        statusline::render_messages(buf, layout.messages, messages, &status.default_face);

        let cmd_rect = prompt.then(|| popup::render_cmdline(buf, screen, status, theme, icons));
        let menu_rect =
            focused.menu.as_ref().filter(|m| !m.items.is_empty()).map(|m| match (m.style.as_str(), cmd_rect) {
                ("inline", _) | (_, None) => popup::render_inline_menu(buf, focus_rect, m, theme),
                (_, Some(c)) => popup::render_prompt_menu(buf, c, screen, m, theme),
            });
        if let Some(info) = &focused.info {
            if !prompt && popup::is_key_help(info) {
                popup::render_whichkey(buf, layout.main, info, theme, icons);
            } else {
                let at = popup::InfoAnchors { menu: menu_rect, cmdline: cmd_rect, cursor: focused.editor.cursor };
                popup::render_info(buf, focus_rect, screen, info, at, theme);
            }
        }
        popup::render_toasts(buf, layout.main, &self.toasts, theme, icons);

        if let (Some(t), true) = (&mut self.term, self.term_visible) {
            t.render(buf, screen, theme);
        }
        if let Some(t) = &mut self.tool {
            t.render(buf, screen, theme);
        }
        if let Some(input) = &self.input {
            input.render(buf, screen, theme);
        }
        if let Some(p) = &mut self.picker {
            p.render(buf, screen, theme, icons);
        }
    }
}

/// `<c-\>` toggles the terminal from anywhere (legacy terminals report it
/// as `<c-4>`).
fn is_term_toggle(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('\\' | '4'))
}

/// Whether `prog` can be run (a path, or a name found on PATH).
fn on_path(prog: &str) -> bool {
    if prog.contains('/') {
        return Path::new(prog).is_file();
    }
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(prog).is_file()))
}

fn git_branch() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let branch = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && !branch.is_empty()).then_some(branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_rows_and_tree_column() {
        let l = Layout::new(Rect::new(0, 0, 100, 30), None);
        assert_eq!(l.tabline, Rect::new(0, 0, 100, 1));
        assert_eq!(l.main, Rect::new(0, 1, 100, 27));
        assert_eq!((l.status.y, l.messages.y), (28, 29));
        assert!(l.tree.is_none());

        let l = Layout::new(Rect::new(0, 0, 100, 30), Some(32));
        assert_eq!(l.tree, Some(Rect::new(0, 1, 32, 27)));
        assert_eq!(l.main, Rect::new(32, 1, 68, 27));

        let l = Layout::new(Rect::new(0, 0, 60, 30), Some(40));
        assert_eq!(l.tree.unwrap().width, 30, "the tree never takes more than half");
    }

    #[test]
    fn terminal_toggle_key() {
        let k = |c, m| is_term_toggle(&KeyEvent::new(KeyCode::Char(c), m));
        assert!(k('\\', KeyModifiers::CONTROL));
        assert!(k('4', KeyModifiers::CONTROL));
        assert!(!k('\\', KeyModifiers::NONE));
    }
}
