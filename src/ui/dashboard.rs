//! Start screen shown when neokak opens without files.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{fill, put, width};
use super::icons::Icons;
use super::theme::Theme;

const LOGO: &[&str] = &[
    "███╗   ██╗███████╗ ██████╗ ██╗  ██╗ █████╗ ██╗  ██╗",
    "████╗  ██║██╔════╝██╔═══██╗██║ ██╔╝██╔══██╗██║ ██╔╝",
    "██╔██╗ ██║█████╗  ██║   ██║█████╔╝ ███████║█████╔╝ ",
    "██║╚██╗██║██╔══╝  ██║   ██║██╔═██╗ ██╔══██║██╔═██╗ ",
    "██║ ╚████║███████╗╚██████╔╝██║  ██╗██║  ██║██║  ██╗",
    "╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝",
];

#[derive(Clone, Copy)]
pub enum DashAction {
    Picker(&'static str),
    Explorer,
    NewFile,
    Config,
    Quit,
}

struct Button {
    key: char,
    icon: &'static str,
    label: &'static str,
    action: DashAction,
}

fn buttons(icons: &Icons) -> [Button; 7] {
    let i = |nerd: &'static str| if icons.nerd { nerd } else { "" };
    [
        Button { key: 'f', icon: i("\u{f002} "), label: "Find file", action: DashAction::Picker("files") },
        Button { key: 'n', icon: i("\u{f15b} "), label: "New file", action: DashAction::NewFile },
        Button { key: 'r', icon: i("\u{f1da} "), label: "Recent files", action: DashAction::Picker("recent") },
        Button { key: 'g', icon: i("\u{f0b0} "), label: "Find text", action: DashAction::Picker("grep") },
        Button { key: 'e', icon: i("\u{f07b} "), label: "File explorer", action: DashAction::Explorer },
        Button { key: 'c', icon: i("\u{f013} "), label: "Config", action: DashAction::Config },
        Button { key: 'q', icon: i("\u{f426} "), label: "Quit", action: DashAction::Quit },
    ]
}

pub fn action_for(key: &KeyEvent, icons: &Icons) -> Option<DashAction> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    let KeyCode::Char(c) = key.code else { return None };
    buttons(icons).into_iter().find(|b| b.key == c).map(|b| b.action)
}

pub fn render(buf: &mut Buffer, area: Rect, theme: &Theme, icons: &Icons) {
    fill(buf, area, Style::new().fg(theme.fg).bg(theme.bg));
    let btns = buttons(icons);
    let logo_w = width(LOGO[0]) as u16;
    let show_logo = area.width > logo_w + 2;
    let height = if show_logo { LOGO.len() as u16 + 2 } else { 0 } + btns.len() as u16 * 2 + 2;
    let mut y = area.y + area.height.saturating_sub(height) / 2;

    if show_logo {
        let x = area.x + (area.width - logo_w) / 2;
        for line in LOGO {
            put(buf, x, y, area.right(), line, Style::new().fg(theme.blue).add_modifier(Modifier::BOLD));
            y += 1;
        }
        y += 2;
    }

    let row_w = 40u16.min(area.width);
    let x = area.x + (area.width - row_w) / 2;
    for b in &btns {
        if y >= area.bottom() {
            return;
        }
        let nx = put(buf, x, y, x + row_w, b.icon, Style::new().fg(theme.aqua));
        put(buf, nx, y, x + row_w, b.label, Style::new().fg(theme.fg));
        put(
            buf,
            x + row_w - 1,
            y,
            x + row_w,
            &b.key.to_string(),
            Style::new().fg(theme.orange).add_modifier(Modifier::BOLD),
        );
        y += 2;
    }
    let footer = format!("neokak v{} · kakoune", env!("CARGO_PKG_VERSION"));
    let fx = area.x + (area.width.saturating_sub(width(&footer) as u16)) / 2;
    put(
        buf,
        fx,
        (y + 1).min(area.bottom().saturating_sub(1)),
        area.right(),
        &footer,
        Style::new().fg(theme.fg_dim).add_modifier(Modifier::ITALIC),
    );
}
