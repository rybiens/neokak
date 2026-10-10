//! Floating windows for Kakoune's menus, info boxes and prompt, plus toasts.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{
    Seg, atom_segs, clamp, float, float_with, put, put_segs, readable, scroll_to, segs_width, width, wrap,
};
use super::icons::Icons;
use super::theme::Theme;
use crate::kak::rpc::{Coord, Face, Line, line_text};

pub struct Menu {
    pub items: Vec<Line>,
    pub anchor: Coord,
    pub style: String,
    pub selected: Option<usize>,
}

pub struct Info {
    pub title: Line,
    pub content: Vec<Line>,
    pub anchor: Coord,
    pub style: String,
}

#[derive(Default)]
pub struct Status {
    pub prompt: Line,
    pub content: Line,
    pub cursor_pos: i32,
    pub mode_line: Line,
    pub default_face: Face,
    pub style: String,
}

impl Status {
    pub fn is_prompt(&self) -> bool {
        matches!(self.style.as_str(), "command" | "search" | "prompt")
    }
}

pub struct Toast {
    pub level: String,
    pub message: String,
    pub until: Instant,
}

const MENU_ROWS: usize = 10;

/// Noice-style floating command line. Returns its frame.
pub fn render_cmdline(buf: &mut Buffer, screen: Rect, status: &Status, theme: &Theme, icons: &Icons) -> Rect {
    let w = (screen.width * 6 / 10).clamp(40.min(screen.width), 80.min(screen.width));
    let area = clamp(Rect::new(screen.x + (screen.width - w) / 2, screen.y + screen.height / 5, w, 3), screen);
    let (icon, title) = match status.style.as_str() {
        "command" => (icons.cmdline(), "Cmdline"),
        "search" => (icons.search(), "Search"),
        _ => (icons.prompt(), "Input"),
    };
    let border = theme.border().fg(theme.yellow);
    let inner = float_with(buf, area, title, theme, border);
    let base = theme.base_face();
    let x = put(buf, inner.x + 1, inner.y, inner.right(), icon, theme.float().fg(theme.yellow));
    let x = put_segs(buf, x, inner.y, inner.right(), &atom_segs(&status.prompt, &base, Some(theme.bg_float)));
    let avail = inner.right().saturating_sub(x + 1) as usize;
    let content = atom_segs(&status.content, &base, Some(theme.bg_float));
    let content = scroll_to(&content, status.cursor_pos.max(0) as usize, avail);
    put_segs(buf, x, inner.y, inner.right() - 1, &content);
    area
}

/// Completion list for the prompt, directly under the cmdline.
pub fn render_prompt_menu(buf: &mut Buffer, under: Rect, screen: Rect, menu: &Menu, theme: &Theme) -> Rect {
    let rows = menu.items.len().min(MENU_ROWS) as u16;
    let area = clamp(Rect::new(under.x, under.bottom(), under.width, rows + 2), screen);
    render_menu_items(buf, area, menu, theme);
    area
}

/// Completion popup anchored at the cursor (insert-mode completion).
pub fn render_inline_menu(buf: &mut Buffer, editor: Rect, menu: &Menu, theme: &Theme) -> Rect {
    let base = theme.base_face();
    let item_w = menu.items.iter().map(|l| segs_width(&atom_segs(l, &base, None))).max().unwrap_or(0);
    let w = (item_w.min(60) + 4) as u16;
    let h = menu.items.len().min(MENU_ROWS) as u16 + 2;
    let area = place_near(editor, menu.anchor, w, h, false);
    render_menu_items(buf, area, menu, theme);
    area
}

fn render_menu_items(buf: &mut Buffer, area: Rect, menu: &Menu, theme: &Theme) {
    let inner = float(buf, area, "", theme);
    let rows = inner.height as usize;
    if rows == 0 {
        return;
    }
    let sel = menu.selected;
    let first = sel.map_or(0, |s| s.saturating_sub(rows - 1));
    let base = theme.base_face();
    for (i, item) in menu.items.iter().enumerate().skip(first).take(rows) {
        let y = inner.y + (i - first) as u16;
        let bg = if Some(i) == sel { theme.bg_sel } else { theme.bg_float };
        buf.set_style(Rect::new(inner.x, y, inner.width, 1), Style::new().bg(bg));
        let mut segs = readable(atom_segs(item, &base, Some(bg)), theme.fg_dim);
        if Some(i) == sel {
            for (_, s) in &mut segs {
                *s = s.add_modifier(Modifier::BOLD);
            }
        }
        put_segs(buf, inner.x + 1, y, inner.right().saturating_sub(1), &segs);
    }
    // Scrollbar on the right border.
    let n = menu.items.len();
    if n > rows {
        let thumb = ((rows * rows) / n).max(1);
        let top = first * (rows - thumb) / (n - rows).max(1);
        for r in 0..thumb {
            put(
                buf,
                area.right() - 1,
                inner.y + (top + r) as u16,
                area.right(),
                "\u{2503}",
                theme.border().fg(theme.fg_dim),
            );
        }
    }
}

/// Chooses a w×h rect next to an anchor in editor coordinates.
fn place_near(editor: Rect, anchor: Coord, w: u16, h: u16, prefer_above: bool) -> Rect {
    let ax = editor.x + anchor.column.max(0) as u16;
    let ay = editor.y + anchor.line.max(0) as u16;
    let below_fits = ay + 1 + h <= editor.bottom();
    let above_fits = ay >= editor.y + h;
    let y = if (prefer_above && above_fits) || (!below_fits && above_fits) { ay - h } else { ay + 1 };
    clamp(Rect::new(ax.saturating_sub(1), y, w, h), editor)
}

/// Kakoune info boxes: hover docs, command help, menu docs.
pub fn render_info(
    buf: &mut Buffer,
    editor: Rect,
    screen: Rect,
    info: &Info,
    menu_rect: Option<Rect>,
    cmdline_rect: Option<Rect>,
    theme: &Theme,
) {
    let base = theme.base_face();
    let lines: Vec<Vec<Seg>> =
        info.content.iter().map(|l| readable(atom_segs(l, &base, Some(theme.bg_float)), theme.fg_dim)).collect();
    let title = line_text(&info.title);
    let wrap_all = |w: usize| -> Vec<Vec<Seg>> { lines.iter().flat_map(|l| wrap(l, w)).collect() };

    // Size for the natural width, then re-wrap to whatever space we get.
    let max_w = (editor.width as usize).min(84).saturating_sub(4).max(10);
    let natural = wrap_all(max_w);
    let text_w = natural.iter().map(|r| segs_width(r)).max().unwrap_or(0).max(width(&title) + 2);
    let w = (text_w + 4) as u16;
    let h = (natural.len().min(editor.height as usize) + 2) as u16;

    let area = match (info.style.as_str(), menu_rect, cmdline_rect) {
        ("menuDoc", Some(m), _) | ("prompt", Some(m), Some(_)) => beside(m, w, h, screen),
        ("prompt", None, Some(c)) => beside(c, w, h, screen),
        ("inlineAbove", ..) => place_near(editor, info.anchor, w, h, true),
        ("inline" | "inlineBelow", ..) => place_near(editor, info.anchor, w, h, false),
        // Fallback for prompt/modal boxes with nothing to attach to.
        _ => clamp(Rect::new(editor.right().saturating_sub(w + 1), editor.bottom().saturating_sub(h), w, h), editor),
    };
    let rows = if area.width == w { natural } else { wrap_all(area.width.saturating_sub(4) as usize) };
    let inner = float(buf, area, &title, theme);
    for (i, row) in rows.iter().take(inner.height as usize).enumerate() {
        put_segs(buf, inner.x + 1, inner.y + i as u16, inner.right().saturating_sub(1), row);
    }
}

/// Next to `r` without covering it: right, else below, else above, shrinking
/// to the space available.
fn beside(r: Rect, w: u16, h: u16, screen: Rect) -> Rect {
    let right = screen.right().saturating_sub(r.right());
    let below = screen.bottom().saturating_sub(r.bottom());
    let above = r.y.saturating_sub(screen.y);
    if right >= w.min(30) {
        clamp(Rect::new(r.right(), r.y, w.min(right), h), screen)
    } else if below >= h.min(5) || below >= above {
        Rect::new(r.x, r.bottom(), w.min(screen.right() - r.x), h.min(below))
    } else {
        let h = h.min(above);
        Rect::new(r.x, r.y - h, w.min(screen.right() - r.x), h)
    }
}

/// Which-key panel for user modes and other key-pending prompts.
pub fn render_whichkey(buf: &mut Buffer, editor: Rect, info: &Info, theme: &Theme, icons: &Icons) {
    let entries: Vec<(String, String)> = info
        .content
        .iter()
        .map(|l| {
            let t = line_text(l);
            match t.split_once(": ") {
                Some((k, d)) => (k.trim().to_owned(), d.trim().to_owned()),
                None => (String::new(), t.trim().to_owned()),
            }
        })
        .collect();
    let mut entries = entries;
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(b.0.cmp(&a.0)));
    let key_w = entries.iter().map(|(k, _)| width(k)).max().unwrap_or(0);
    let desc_w = entries.iter().map(|(_, d)| width(d)).max().unwrap_or(0).min(40);
    let col_w = key_w + width(icons.arrow()) + desc_w + 5;
    let cols = ((editor.width as usize).saturating_sub(4) / col_w.max(1)).max(1);
    let rows = entries.len().div_ceil(cols).max(1);
    let h = (rows as u16 + 2).min(editor.height);
    let area = Rect::new(editor.x, editor.bottom() - h, editor.width, h);

    let title = line_text(&info.title);
    let inner = float(buf, area, &title, theme);
    for (i, (key, desc)) in entries.iter().enumerate() {
        let (c, r) = (i / rows, i % rows);
        if r as u16 >= inner.height {
            continue;
        }
        let x = inner.x + 2 + (c * col_w) as u16;
        let y = inner.y + r as u16;
        let max_x = (x + col_w as u16).min(inner.right());
        let x = put(
            buf,
            x,
            y,
            max_x,
            &format!("{key:>key_w$} "),
            theme.float().fg(theme.orange).add_modifier(Modifier::BOLD),
        );
        let x = put(buf, x, y, max_x, &format!("{} ", icons.arrow()), theme.float().fg(theme.fg_dim));
        let group = desc.starts_with('+');
        let style = if group { theme.float().fg(theme.purple) } else { theme.float() };
        put(buf, x, y, max_x, desc, style);
    }
}

/// nvim-notify style toasts stacked in the top-right corner.
pub fn render_toasts(buf: &mut Buffer, editor: Rect, toasts: &[Toast], theme: &Theme, icons: &Icons) {
    let mut y = editor.y;
    for t in toasts {
        let color = match t.level.as_str() {
            "error" => theme.red,
            "warn" => theme.yellow,
            _ => theme.green,
        };
        let body: Vec<Seg> = vec![(t.message.clone(), theme.float())];
        let max_w = (editor.width as usize / 2).clamp(20, 60);
        let rows = wrap(&body, max_w);
        let w = (rows.iter().map(|r| segs_width(r)).max().unwrap_or(0) + 4).max(20) as u16;
        let h = rows.len() as u16 + 2;
        if y + h > editor.bottom() {
            break;
        }
        let area = Rect::new(editor.right().saturating_sub(w + 1), y, w, h);
        let title = format!("{}{}", icons.level(&t.level), t.level);
        let inner = float_with(buf, area, title.trim(), theme, theme.border().fg(color));
        for (i, row) in rows.iter().enumerate() {
            put_segs(buf, inner.x + 1, inner.y + i as u16, inner.right(), row);
        }
        y += h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kak::rpc::Atom;
    use crate::ui::draw::rows;

    fn line(s: &str) -> Line {
        vec![Atom { face: Face::default(), contents: s.into() }]
    }

    #[test]
    fn places_below_or_above_the_anchor() {
        let editor = Rect::new(0, 1, 80, 20);
        let below = place_near(editor, Coord { line: 2, column: 10 }, 20, 5, false);
        assert_eq!((below.x, below.y), (9, 4));
        let above = place_near(editor, Coord { line: 18, column: 10 }, 20, 5, false);
        assert_eq!(above.y, 14, "no room below, so it flips above");
        let preferred = place_near(editor, Coord { line: 10, column: 0 }, 20, 5, true);
        assert_eq!(preferred.bottom(), editor.y + 10);
    }

    #[test]
    fn beside_avoids_covering_the_target() {
        let target = Rect::new(10, 5, 20, 3);
        let right = beside(target, 40, 10, Rect::new(0, 0, 100, 30));
        assert_eq!((right.x, right.y), (30, 5));
        let below = beside(target, 40, 10, Rect::new(0, 0, 50, 30));
        assert_eq!(below.y, target.bottom());
        assert!(below.intersection(target).is_empty());
    }

    #[test]
    fn whichkey_sorts_and_renders_entries() {
        let info = Info {
            title: line("user"),
            content: vec![line("f: +find"), line("b: +buffer"), line("<space>: find files")],
            anchor: Coord::default(),
            style: "prompt".into(),
        };
        let area = Rect::new(0, 0, 100, 10);
        let mut buf = Buffer::empty(area);
        render_whichkey(&mut buf, area, &info, &Theme::default(), &Icons { nerd: false });
        let text = rows(&buf).join("\n");
        let pos = |s: &str| text.find(s).unwrap_or_else(|| panic!("{s:?} not in {text}"));
        assert!(pos("<space> -> find files") < pos("b -> +buffer"));
        assert!(pos("b -> +buffer") < pos("f -> +find"));
        assert!(text.contains("user"));
    }

    #[test]
    fn cmdline_shows_prompt_and_keeps_cursor_visible() {
        let screen = Rect::new(0, 0, 60, 20);
        let status = Status {
            prompt: line(":"),
            content: line(&format!("edit {}", "x".repeat(100))),
            cursor_pos: 105,
            style: "command".into(),
            ..Status::default()
        };
        let mut buf = Buffer::empty(screen);
        let rect = render_cmdline(&mut buf, screen, &status, &Theme::default(), &Icons { nerd: false });
        let text = rows(&buf).join("\n");
        assert!(text.contains("Cmdline"));
        assert!(text.contains(":xxxx"), "start scrolled away to keep the end in view: {text}");
        assert_eq!(rect.height, 3);
    }

    #[test]
    fn inline_menu_highlights_selection() {
        let editor = Rect::new(0, 0, 40, 10);
        let menu = Menu {
            items: vec![line("alpha"), line("beta")],
            anchor: Coord { line: 1, column: 2 },
            style: "inline".into(),
            selected: Some(1),
        };
        let mut buf = Buffer::empty(editor);
        let theme = Theme::default();
        let rect = render_inline_menu(&mut buf, editor, &menu, &theme);
        let text = rows(&buf);
        let (y, row) = text.iter().enumerate().find(|(_, r)| r.contains("beta")).unwrap();
        let x = row.find("beta").unwrap() as u16;
        assert_eq!(buf[(x, y as u16)].bg, theme.bg_sel);
        assert!(rect.y > 1, "below the anchor line");
    }

    #[test]
    fn info_wraps_long_lines_inside_the_editor() {
        let editor = Rect::new(0, 0, 60, 20);
        let info = Info {
            title: line("hover"),
            content: vec![line(&"word ".repeat(40))],
            anchor: Coord { line: 2, column: 5 },
            style: "inline".into(),
        };
        let mut buf = Buffer::empty(editor);
        render_info(&mut buf, editor, editor, &info, None, None, &Theme::default());
        let wrapped = rows(&buf).iter().filter(|r| r.contains("word")).count();
        assert!(wrapped >= 3, "200 chars in a <60 col box need several rows");
    }
}
