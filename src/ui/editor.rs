//! The buffer viewport Kakoune draws into.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::draw::{atom_segs, fill, put_segs};
use super::theme::face_style;
use crate::kak::rpc::{Face, Line};

#[derive(Default)]
pub struct EditorView {
    pub lines: Vec<Line>,
    pub default_face: Face,
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
