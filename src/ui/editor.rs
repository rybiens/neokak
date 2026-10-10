//! The buffer viewport Kakoune draws into.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::draw::{atom_segs, fill, put_segs};
use super::theme::face_style;
use crate::kak::rpc::{Coord, Face, Line};

#[derive(Default)]
pub struct EditorView {
    pub lines: Vec<Line>,
    pub default_face: Face,
    /// Main cursor, relative to the pane.
    pub cursor: Coord,
}

impl EditorView {
    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        let base = face_style(&self.default_face, &Face::default());
        fill(buf, area, base);
        // Rows past the end of the buffer stay blank (no `~` padding, like
        // most Neovim setups).
        for (row, line) in self.lines.iter().take(area.height as usize).enumerate() {
            let y = area.y + row as u16;
            put_segs(buf, area.x, y, area.right(), &atom_segs(line, &self.default_face, None));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kak::rpc::Atom;

    #[test]
    fn draws_lines_and_blanks_the_rest() {
        let atom = |s: &str| Atom { face: Face::default(), contents: s.into() };
        let view = EditorView {
            lines: vec![vec![atom("hel"), atom("lo\n")]],
            default_face: Face::default(),
            cursor: Coord::default(),
        };
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        view.render(&mut buf, area);
        assert_eq!(crate::ui::draw::rows(&buf), ["hello     ", "          ", "          "]);
    }
}
