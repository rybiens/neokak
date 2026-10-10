//! Bufferline-style tab bar of open buffers.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{fill, put, width};
use super::icons::Icons;
use super::theme::Theme;

/// A clickable tab: columns [start, end) and the buffer it switches to.
pub struct Hit {
    pub start: u16,
    pub end: u16,
    pub buffer: String,
}

/// Buffers worth showing: real files, plus whatever is current.
pub fn visible<'a>(buffers: &'a [String], current: &str) -> Vec<&'a String> {
    buffers.iter().filter(|b| !b.starts_with('*') || *b == current).collect()
}

pub fn render(
    buf: &mut Buffer,
    area: Rect,
    buffers: &[String],
    current: &str,
    modified: bool,
    theme: &Theme,
    icons: &Icons,
) -> Vec<Hit> {
    let bar = Style::new().fg(theme.fg_dim).bg(theme.bg_bar);
    fill(buf, area, bar);

    let tabs: Vec<(String, &String)> = visible(buffers, current)
        .into_iter()
        .map(|b| {
            let short = b.rsplit('/').next().unwrap_or(b);
            let mark = if b == current && modified { format!(" {}", icons.modified()) } else { "  ".into() };
            (format!("  {}{short}{mark} ", icons.file(b)), b)
        })
        .collect();

    // Scroll so the current tab is visible.
    let cur = tabs.iter().position(|(_, b)| *b == current).unwrap_or(0);
    let mut first = 0;
    while first < cur && tabs[first..=cur].iter().map(|(t, _)| width(t)).sum::<usize>() > area.width as usize {
        first += 1;
    }

    let mut hits = Vec::new();
    let mut x = area.x;
    for (text, name) in &tabs[first..] {
        let start = x;
        if *name == current {
            let active = Style::new().fg(theme.fg).bg(theme.bg).add_modifier(Modifier::BOLD);
            put(buf, x, area.y, area.right(), icons.tab_active(), active.fg(theme.blue));
            x = put(buf, x + 1, area.y, area.right(), &text[1..], active);
        } else {
            x = put(buf, x, area.y, area.right(), text, bar);
        }
        hits.push(Hit { start, end: x, buffer: (*name).clone() });
        if x >= area.right() {
            break;
        }
    }
    hits
}

/// A centered label over a sidebar column.
pub fn render_offset(buf: &mut Buffer, area: Rect, label: &str, theme: &Theme) {
    let style = Style::new().fg(theme.blue).bg(theme.bg_bar).add_modifier(Modifier::BOLD);
    fill(buf, area, style);
    let pad = (area.width as usize).saturating_sub(width(label)) / 2;
    put(buf, area.x + pad as u16, area.y, area.right(), label, style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::rows;

    fn bufs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn draw(buffers: &[String], current: &str, width: u16) -> (String, Vec<Hit>) {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let hits = render(&mut buf, area, buffers, current, false, &Theme::default(), &Icons { nerd: false });
        (rows(&buf).join(""), hits)
    }

    #[test]
    fn hides_special_buffers_unless_current() {
        let b = bufs(&["*debug*", "src/a.rs", "src/b.rs"]);
        let (text, hits) = draw(&b, "src/b.rs", 80);
        assert!(!text.contains("debug"));
        assert_eq!(hits.iter().map(|h| h.buffer.as_str()).collect::<Vec<_>>(), ["src/a.rs", "src/b.rs"]);
        assert!(hits[0].end <= hits[1].start, "tabs don't overlap");
        let (text, _) = draw(&b, "*debug*", 80);
        assert!(text.contains("*debug*"));
    }

    #[test]
    fn scrolls_to_keep_current_visible() {
        let names: Vec<String> = (0..20).map(|i| format!("file{i}.rs")).collect();
        let (_, hits) = draw(&names, "file19.rs", 40);
        assert!(hits.iter().any(|h| h.buffer == "file19.rs"));
        assert!(!hits.iter().any(|h| h.buffer == "file0.rs"));
    }
}
