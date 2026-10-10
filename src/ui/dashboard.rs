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
    RestoreSession,
    Quit,
}

struct Button {
    key: char,
    icon: &'static str,
    label: &'static str,
    action: DashAction,
}

fn buttons(icons: &Icons) -> [Button; 9] {
    let i = |nerd: &'static str| if icons.nerd { nerd } else { "" };
    [
        Button { key: 'f', icon: i("\u{f002} "), label: "Find file", action: DashAction::Picker("files") },
        Button { key: 'n', icon: i("\u{f15b} "), label: "New file", action: DashAction::NewFile },
        Button { key: 's', icon: i("\u{f0c7} "), label: "Restore session", action: DashAction::RestoreSession },
        Button { key: 'p', icon: i("\u{f503} "), label: "Projects", action: DashAction::Picker("projects") },
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
    let buttons_h = btns.len() as u16 * 2 + 2;
    let logo_h = LOGO.len() as u16 + 2;
    // The logo is the first thing to go when space is short.
    let show_logo = area.width > logo_w + 2 && area.height >= logo_h + buttons_h;
    let height = if show_logo { logo_h } else { 0 } + buttons_h;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::rows;

    #[test]
    fn buttons_map_to_actions() {
        let icons = Icons { nerd: false };
        let k = |c, m| action_for(&KeyEvent::new(KeyCode::Char(c), m), &icons);
        assert!(matches!(k('f', KeyModifiers::NONE), Some(DashAction::Picker("files"))));
        assert!(matches!(k('g', KeyModifiers::NONE), Some(DashAction::Picker("grep"))));
        assert!(matches!(k('q', KeyModifiers::NONE), Some(DashAction::Quit)));
        assert!(matches!(k('s', KeyModifiers::NONE), Some(DashAction::RestoreSession)));
        assert!(matches!(k('p', KeyModifiers::NONE), Some(DashAction::Picker("projects"))));
        assert!(k('f', KeyModifiers::CONTROL).is_none());
        assert!(k('x', KeyModifiers::NONE).is_none());
    }

    #[test]
    fn short_windows_drop_the_logo_not_the_buttons() {
        let area = Rect::new(0, 0, 80, 22);
        let mut buf = Buffer::empty(area);
        render(&mut buf, area, &Theme::default(), &Icons { nerd: false });
        let text = rows(&buf).join("\n");
        assert!(!text.contains("███╗"));
        assert!(text.contains("Quit"), "{text}");
    }

    #[test]
    fn renders_logo_and_buttons() {
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        render(&mut buf, area, &Theme::default(), &Icons { nerd: false });
        let text = rows(&buf).join("\n");
        assert!(text.contains("███╗"), "logo");
        assert!(text.contains("Find file") && text.contains("Restore session") && text.contains("Quit"), "{text}");
    }
}
