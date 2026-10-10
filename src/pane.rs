//! Editor panes (one Kakoune client each) and the split layout tree.

use ratatui::layout::Rect;

use crate::kak::client::KakClient;
use crate::kak::rpc::KakEvent;
use crate::ui::editor::EditorView;
use crate::ui::popup::{Info, Menu, Status};

pub type PaneId = usize;

pub struct Pane {
    pub id: PaneId,
    /// Kakoune client name, used for `eval -client`.
    pub client: String,
    pub kak: KakClient,
    pub editor: EditorView,
    pub status: Status,
    pub menu: Option<Menu>,
    pub info: Option<Info>,
    /// Buffer shown in this pane.
    pub current: String,
    size: (u16, u16),
    /// Kakoune can drop a resize that arrives during startup; resend it once
    /// the first frame shows up.
    first_frame: bool,
}

impl Pane {
    pub fn new(id: PaneId, client: String, kak: KakClient) -> Self {
        Self {
            id,
            client,
            kak,
            editor: EditorView::default(),
            status: Status::default(),
            menu: None,
            info: None,
            current: String::new(),
            size: (0, 0),
            first_frame: true,
        }
    }

    pub fn resize(&mut self, area: Rect) {
        let size = (area.height, area.width);
        if size != self.size && size.0 > 0 && size.1 > 0 {
            self.kak.resize(size.0, size.1);
            self.size = size;
        }
    }

    pub fn handle_kak(&mut self, ev: KakEvent) {
        match ev {
            KakEvent::Draw { lines, cursor, default_face, .. } => {
                if std::mem::take(&mut self.first_frame) && self.size.0 > 0 {
                    self.kak.resize(self.size.0, self.size.1);
                }
                self.editor.lines = lines;
                self.editor.default_face = default_face;
                self.editor.cursor = cursor;
            }
            KakEvent::DrawStatus { prompt, content, cursor_pos, mode_line, default_face, style } => {
                self.status = Status { prompt, content, cursor_pos, mode_line, default_face, style };
            }
            KakEvent::MenuShow { items, anchor, style, .. } => {
                self.menu = Some(Menu { items, anchor, style, selected: None });
            }
            KakEvent::MenuSelect(i) => {
                if let Some(m) = &mut self.menu {
                    m.selected = usize::try_from(i).ok().filter(|&i| i < m.items.len());
                }
            }
            KakEvent::MenuHide => self.menu = None,
            KakEvent::InfoShow { title, content, anchor, style, .. } => {
                self.info = Some(Info { title, content, anchor, style });
            }
            KakEvent::InfoHide => self.info = None,
            KakEvent::Refresh(_) | KakEvent::SetUiOptions(_) | KakEvent::Unknown(_) => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Side by side.
    Vertical,
    /// Stacked.
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Split {
    Leaf(PaneId),
    Node(Dir, Vec<Split>),
}

impl Split {
    /// Splits the pane `at`, putting `new` after it.
    pub fn split(&mut self, at: PaneId, dir: Dir, new: PaneId) -> bool {
        match self {
            Split::Leaf(id) if *id == at => {
                *self = Split::Node(dir, vec![Split::Leaf(at), Split::Leaf(new)]);
                true
            }
            Split::Leaf(_) => false,
            Split::Node(d, children) => {
                if *d == dir
                    && let Some(i) = children.iter().position(|c| *c == Split::Leaf(at))
                {
                    children.insert(i + 1, Split::Leaf(new));
                    return true;
                }
                children.iter_mut().any(|c| c.split(at, dir, new))
            }
        }
    }

    /// Removes a pane, collapsing nodes left with one child.
    pub fn remove(&mut self, id: PaneId) {
        if let Split::Node(_, children) = self {
            children.retain(|c| *c != Split::Leaf(id));
            for c in children.iter_mut() {
                c.remove(id);
            }
            if children.len() == 1 {
                *self = children.pop().unwrap();
            }
        }
    }

    pub fn ids(&self) -> Vec<PaneId> {
        match self {
            Split::Leaf(id) => vec![*id],
            Split::Node(_, c) => c.iter().flat_map(Split::ids).collect(),
        }
    }

    /// Pane rects plus separator rects, with one-cell separators between
    /// siblings.
    pub fn layout(&self, area: Rect, panes: &mut Vec<(PaneId, Rect)>, seps: &mut Vec<(Dir, Rect)>) {
        match self {
            Split::Leaf(id) => panes.push((*id, area)),
            Split::Node(dir, children) => {
                let n = children.len() as u16;
                let total = match dir {
                    Dir::Vertical => area.width,
                    Dir::Horizontal => area.height,
                };
                let avail = total.saturating_sub(n - 1);
                let mut offset = 0;
                for (i, child) in children.iter().enumerate() {
                    let i = i as u16;
                    // Spread the remainder over the first panes.
                    let size = avail / n + u16::from(i < avail % n);
                    let rect = match dir {
                        Dir::Vertical => Rect { x: area.x + offset, width: size, ..area },
                        Dir::Horizontal => Rect { y: area.y + offset, height: size, ..area },
                    };
                    child.layout(rect, panes, seps);
                    offset += size;
                    if i + 1 < n {
                        let sep = match dir {
                            Dir::Vertical => Rect { x: area.x + offset, width: 1, ..area },
                            Dir::Horizontal => Rect { y: area.y + offset, height: 1, ..area },
                        };
                        seps.push((*dir, sep));
                        offset += 1;
                    }
                }
            }
        }
    }
}

/// The pane whose rect lies in direction (dx, dy) from `from`, nearest first.
pub fn neighbor(rects: &[(PaneId, Rect)], from: PaneId, dx: i32, dy: i32) -> Option<PaneId> {
    let (_, r) = rects.iter().find(|(id, _)| *id == from)?;
    let center = |r: &Rect| (r.x as i32 * 2 + r.width as i32, r.y as i32 * 2 + r.height as i32);
    let (cx, cy) = center(r);
    rects
        .iter()
        .filter(|(id, o)| {
            *id != from
                && match (dx, dy) {
                    (1, _) => o.x >= r.right(),
                    (-1, _) => o.right() <= r.x,
                    (_, 1) => o.y >= r.bottom(),
                    _ => o.bottom() <= r.y,
                }
        })
        .min_by_key(|(_, o)| {
            let (ox, oy) = center(o);
            // Prefer panes that overlap on the other axis.
            (ox - cx).abs() * if dx != 0 { 1 } else { 4 } + (oy - cy).abs() * if dy != 0 { 1 } else { 4 }
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_remove() {
        let mut s = Split::Leaf(0);
        assert!(s.split(0, Dir::Vertical, 1));
        assert!(s.split(1, Dir::Vertical, 2));
        assert_eq!(s, Split::Node(Dir::Vertical, vec![Split::Leaf(0), Split::Leaf(1), Split::Leaf(2)]));
        assert!(s.split(1, Dir::Horizontal, 3));
        assert_eq!(s.ids(), vec![0, 1, 3, 2]);
        s.remove(3);
        s.remove(2);
        assert_eq!(s, Split::Node(Dir::Vertical, vec![Split::Leaf(0), Split::Leaf(1)]));
        s.remove(1);
        assert_eq!(s, Split::Leaf(0));
    }

    #[test]
    fn layout_with_separators() {
        let s = Split::Node(Dir::Vertical, vec![Split::Leaf(0), Split::Leaf(1)]);
        let (mut panes, mut seps) = (Vec::new(), Vec::new());
        s.layout(Rect::new(0, 0, 21, 10), &mut panes, &mut seps);
        assert_eq!(panes, vec![(0, Rect::new(0, 0, 10, 10)), (1, Rect::new(11, 0, 10, 10))]);
        assert_eq!(seps, vec![(Dir::Vertical, Rect::new(10, 0, 1, 10))]);
        assert_eq!(neighbor(&panes, 0, 1, 0), Some(1));
        assert_eq!(neighbor(&panes, 1, -1, 0), Some(0));
        assert_eq!(neighbor(&panes, 0, 0, 1), None);
    }
}
