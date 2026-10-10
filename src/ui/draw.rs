//! Low-level drawing helpers shared by the UI components.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Clear, Widget};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::theme::{Theme, face_style};
use crate::kak::rpc::{Atom, Face};

/// A run of text with one style.
pub type Seg = (String, Style);

/// Replaces characters that would corrupt the terminal grid.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\t' => ' ',
            c if c.is_control() => '?',
            c => c,
        })
        .collect()
}

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Writes `text` at (x, y), never past `max_x`. Returns the next free column.
pub fn put(buf: &mut Buffer, x: u16, y: u16, max_x: u16, text: &str, style: Style) -> u16 {
    if x >= max_x || y >= buf.area.bottom() {
        return x;
    }
    buf.set_stringn(x, y, sanitize(text), (max_x - x) as usize, style).0
}

pub fn put_segs(buf: &mut Buffer, mut x: u16, y: u16, max_x: u16, segs: &[Seg]) -> u16 {
    for (text, style) in segs {
        x = put(buf, x, y, max_x, text, *style);
    }
    x
}

/// Converts atoms to segments, optionally forcing a background color.
pub fn atom_segs(atoms: &[Atom], base: &Face, bg: Option<Color>) -> Vec<Seg> {
    atoms
        .iter()
        .map(|a| {
            let mut style = face_style(&a.face, base);
            if let Some(bg) = bg {
                // Keep explicit highlights (e.g. matched text), replace defaults.
                if a.face.bg.is_empty() || a.face.bg == "default" {
                    style = style.bg(bg);
                }
            }
            (a.contents.clone(), style)
        })
        .collect()
}

/// Replaces foregrounds that would vanish on the theme's float background
/// (colorschemes tune faces like MenuInfo for Kakoune's own menu
/// background, not ours): too dark on dark themes, too light on light ones.
pub fn readable(mut segs: Vec<Seg>, theme: &Theme) -> Vec<Seg> {
    for (_, style) in &mut segs {
        if let Some(Color::Rgb(r, g, b)) = style.fg {
            let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
            if (theme.dark && luma < 70.0) || (!theme.dark && luma > 200.0) {
                style.fg = Some(theme.fg_dim);
            }
        }
    }
    segs
}

pub fn segs_width(segs: &[Seg]) -> usize {
    segs.iter().map(|(t, _)| width(t)).sum()
}

/// Hard-wraps segments into rows of at most `max` columns.
pub fn wrap(segs: &[Seg], max: usize) -> Vec<Vec<Seg>> {
    let max = max.max(1);
    let mut rows = vec![Vec::new()];
    let mut col = 0;
    for (text, style) in segs {
        let mut cur = String::new();
        for c in sanitize(text).chars() {
            let w = c.width().unwrap_or(0);
            if col + w > max {
                if !cur.is_empty() {
                    rows.last_mut().unwrap().push((std::mem::take(&mut cur), *style));
                }
                rows.push(Vec::new());
                col = 0;
            }
            cur.push(c);
            col += w;
        }
        if !cur.is_empty() {
            rows.last_mut().unwrap().push((cur, *style));
        }
    }
    rows
}

/// Drops leading columns so that column `keep` stays within `max` columns.
pub fn scroll_to(segs: &[Seg], keep: usize, max: usize) -> Vec<Seg> {
    if keep < max {
        return segs.to_vec();
    }
    let mut skip = keep + 1 - max;
    let mut out = Vec::new();
    for (text, style) in segs {
        let mut kept = String::new();
        for c in text.chars() {
            let w = c.width().unwrap_or(0);
            if skip >= w && skip > 0 {
                skip -= w;
            } else {
                skip = 0;
                kept.push(c);
            }
        }
        if !kept.is_empty() {
            out.push((kept, *style));
        }
    }
    out
}

/// A rounded floating window frame; returns its inner area.
pub fn float(buf: &mut Buffer, area: Rect, title: &str, theme: &Theme) -> Rect {
    float_with(buf, area, title, theme, theme.border())
}

pub fn float_with(buf: &mut Buffer, area: Rect, title: &str, theme: &Theme, border: Style) -> Rect {
    let area = area.intersection(buf.area);
    Clear.render(area, buf);
    let mut block = Block::bordered().border_type(BorderType::Rounded).border_style(border).style(theme.float());
    if !title.is_empty() {
        block = block.title(Span::styled(format!(" {title} "), border.fg(theme.fg)));
    }
    let inner = block.inner(area);
    block.render(area, buf);
    inner
}

pub fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    let area = area.intersection(buf.area);
    Clear.render(area, buf);
    buf.set_style(area, style);
}

/// Clamps a rect so it lies inside `bounds`, shifting before shrinking.
pub fn clamp(mut r: Rect, bounds: Rect) -> Rect {
    r.width = r.width.min(bounds.width);
    r.height = r.height.min(bounds.height);
    r.x = r.x.max(bounds.x).min(bounds.right() - r.width);
    r.y = r.y.max(bounds.y).min(bounds.bottom() - r.height);
    r
}

/// The text of each row of a rendered buffer (for tests).
#[cfg(test)]
pub(crate) fn rows(buf: &Buffer) -> Vec<String> {
    (buf.area.y..buf.area.bottom())
        .map(|y| (buf.area.x..buf.area.right()).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(rows: &[Vec<Seg>]) -> Vec<String> {
        rows.iter().map(|r| r.iter().map(|(t, _)| t.as_str()).collect()).collect()
    }

    #[test]
    fn wraps() {
        let segs = vec![("abcdef".to_string(), Style::new()), ("gh".to_string(), Style::new())];
        assert_eq!(text(&wrap(&segs, 3)), ["abc", "def", "gh"]);
    }

    #[test]
    fn scrolls() {
        let segs = vec![("abcdef".to_string(), Style::new())];
        assert_eq!(text(&[scroll_to(&segs, 5, 3)]), ["def"]);
        assert_eq!(text(&[scroll_to(&segs, 1, 3)]), ["abcdef"]);
    }

    #[test]
    fn clamps() {
        let b = Rect::new(0, 0, 10, 10);
        assert_eq!(clamp(Rect::new(8, 8, 4, 4), b), Rect::new(6, 6, 4, 4));
    }
}
