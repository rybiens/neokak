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
    /// Children with their relative sizes (weights).
    Node(Dir, Vec<(Split, u32)>),
}

/// A separator between two siblings: `index` is the child before it, and
/// `path` the child indices leading from the root to its node.
#[derive(Debug, Clone, PartialEq)]
pub struct Separator {
    pub dir: Dir,
    pub rect: Rect,
    pub path: Vec<usize>,
    pub index: usize,
}

/// Panes never shrink below this many cells along a split.
const MIN_SIZE: u16 = 3;

fn along(dir: Dir, r: Rect) -> u16 {
    match dir {
        Dir::Vertical => r.width,
        Dir::Horizontal => r.height,
    }
}

fn start(dir: Dir, r: Rect) -> u16 {
    match dir {
        Dir::Vertical => r.x,
        Dir::Horizontal => r.y,
    }
}

/// Splits `total` cells among weights, after one-cell separators.
fn sizes(total: u16, weights: &[u32]) -> Vec<u16> {
    let n = weights.len() as u16;
    let avail = total.saturating_sub(n.saturating_sub(1)) as u64;
    let sum: u64 = weights.iter().map(|&w| w.max(1) as u64).sum::<u64>().max(1);
    let mut out: Vec<u16> = weights.iter().map(|&w| (avail * w.max(1) as u64 / sum) as u16).collect();
    // Give the rounding remainder to the first panes.
    let mut rest = avail as u16 - out.iter().sum::<u16>();
    for s in out.iter_mut() {
        if rest == 0 {
            break;
        }
        *s += 1;
        rest -= 1;
    }
    out
}

/// Child rects of a node.
fn child_rects(dir: Dir, area: Rect, weights: &[u32]) -> Vec<Rect> {
    let mut offset = start(dir, area);
    sizes(along(dir, area), weights)
        .into_iter()
        .map(|size| {
            let r = match dir {
                Dir::Vertical => Rect { x: offset, width: size, ..area },
                Dir::Horizontal => Rect { y: offset, height: size, ..area },
            };
            offset += size + 1;
            r
        })
        .collect()
}

impl Split {
    /// Splits the pane `at`, putting `new` after it with an equal share.
    pub fn split(&mut self, at: PaneId, dir: Dir, new: PaneId) -> bool {
        match self {
            Split::Leaf(id) if *id == at => {
                *self = Split::Node(dir, vec![(Split::Leaf(at), 1), (Split::Leaf(new), 1)]);
                true
            }
            Split::Leaf(_) => false,
            Split::Node(d, children) => {
                if *d == dir
                    && let Some(i) = children.iter().position(|(c, _)| *c == Split::Leaf(at))
                {
                    // Halve the split pane to make room.
                    let w = children[i].1;
                    children[i].1 = w.div_ceil(2).max(1);
                    children.insert(i + 1, (Split::Leaf(new), (w / 2).max(1)));
                    return true;
                }
                children.iter_mut().any(|(c, _)| c.split(at, dir, new))
            }
        }
    }

    /// Removes a pane, collapsing nodes left with one child. Its space goes
    /// to its siblings.
    pub fn remove(&mut self, id: PaneId) {
        if let Split::Node(_, children) = self {
            children.retain(|(c, _)| *c != Split::Leaf(id));
            for (c, _) in children.iter_mut() {
                c.remove(id);
            }
            if children.len() == 1 {
                *self = children.pop().unwrap().0;
            }
        }
    }

    pub fn ids(&self) -> Vec<PaneId> {
        match self {
            Split::Leaf(id) => vec![*id],
            Split::Node(_, c) => c.iter().flat_map(|(s, _)| s.ids()).collect(),
        }
    }

    /// Resets every split to equal sizes.
    pub fn equalize(&mut self) {
        if let Split::Node(_, children) = self {
            for (c, w) in children.iter_mut() {
                *w = 1;
                c.equalize();
            }
        }
    }

    /// Pane rects plus separators, with one-cell separators between siblings.
    pub fn layout(&self, area: Rect, panes: &mut Vec<(PaneId, Rect)>, seps: &mut Vec<Separator>) {
        self.layout_at(area, &mut Vec::new(), panes, seps);
    }

    fn layout_at(&self, area: Rect, path: &mut Vec<usize>, panes: &mut Vec<(PaneId, Rect)>, seps: &mut Vec<Separator>) {
        match self {
            Split::Leaf(id) => panes.push((*id, area)),
            Split::Node(dir, children) => {
                let weights: Vec<u32> = children.iter().map(|(_, w)| *w).collect();
                let rects = child_rects(*dir, area, &weights);
                for (i, ((child, _), rect)) in children.iter().zip(&rects).enumerate() {
                    path.push(i);
                    child.layout_at(*rect, path, panes, seps);
                    path.pop();
                    if i + 1 < children.len() {
                        let rect = match dir {
                            Dir::Vertical => Rect { x: rect.right(), width: 1, ..area },
                            Dir::Horizontal => Rect { y: rect.bottom(), height: 1, ..area },
                        };
                        seps.push(Separator { dir: *dir, rect, path: path.clone(), index: i });
                    }
                }
            }
        }
    }

    /// Grows (or with a negative `delta`, shrinks) pane `id` by `delta`
    /// cells along `dir`, taking the space from its neighbor in the nearest
    /// enclosing split of that direction.
    pub fn resize(&mut self, area: Rect, id: PaneId, dir: Dir, delta: i32) -> bool {
        let Split::Node(d, children) = self else { return false };
        let weights: Vec<u32> = children.iter().map(|(_, w)| *w).collect();
        let rects = child_rects(*d, area, &weights);
        let Some(k) = children.iter().position(|(c, _)| c.ids().contains(&id)) else { return false };
        if children[k].0.resize(rects[k], id, dir, delta) {
            return true;
        }
        if *d != dir || children.len() < 2 {
            return false;
        }
        let other = if k + 1 < children.len() { k + 1 } else { k - 1 };
        let mut cells: Vec<i32> = rects.iter().map(|r| along(dir, *r) as i32).collect();
        let delta = delta.clamp(MIN_SIZE as i32 - cells[k], cells[other] - MIN_SIZE as i32);
        if delta == 0 {
            return false;
        }
        cells[k] += delta;
        cells[other] -= delta;
        for ((_, w), c) in children.iter_mut().zip(cells) {
            *w = c.max(1) as u32;
        }
        true
    }

    /// Moves a separator (from [`Split::layout`]) to screen position `pos`
    /// along its direction, e.g. while dragging it with the mouse.
    pub fn drag(&mut self, area: Rect, sep: &Separator, pos: u16) {
        let mut node = self;
        let mut area = area;
        for &i in &sep.path {
            let Split::Node(d, children) = node else { return };
            let weights: Vec<u32> = children.iter().map(|(_, w)| *w).collect();
            area = child_rects(*d, area, &weights)[i];
            node = &mut children[i].0;
        }
        let Split::Node(d, children) = node else { return };
        let weights: Vec<u32> = children.iter().map(|(_, w)| *w).collect();
        let rects = child_rects(*d, area, &weights);
        let (a, b) = (sep.index, sep.index + 1);
        if b >= rects.len() {
            return;
        }
        let pair = along(*d, rects[a]) + along(*d, rects[b]);
        let size_a =
            pos.saturating_sub(start(*d, rects[a])).clamp(MIN_SIZE, pair.saturating_sub(MIN_SIZE).max(MIN_SIZE));
        let mut cells: Vec<u16> = rects.iter().map(|r| along(*d, *r)).collect();
        cells[a] = size_a;
        cells[b] = pair - size_a.min(pair);
        for ((_, w), c) in children.iter_mut().zip(cells) {
            *w = c.max(1) as u32;
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
        assert_eq!(s.ids(), vec![0, 1, 2]);
        assert!(matches!(&s, Split::Node(Dir::Vertical, c) if c.len() == 3), "same-direction splits flatten");
        assert!(s.split(1, Dir::Horizontal, 3));
        assert_eq!(s.ids(), vec![0, 1, 3, 2]);
        s.remove(3);
        s.remove(2);
        assert_eq!(s.ids(), vec![0, 1]);
        s.remove(1);
        assert_eq!(s, Split::Leaf(0));
    }

    fn rects(s: &Split, area: Rect) -> Vec<(PaneId, Rect)> {
        let (mut panes, mut seps) = (Vec::new(), Vec::new());
        s.layout(area, &mut panes, &mut seps);
        panes
    }

    #[test]
    fn resize_moves_space_between_neighbors() {
        let area = Rect::new(0, 0, 41, 10);
        let mut s = Split::Leaf(0);
        s.split(0, Dir::Vertical, 1);
        assert!(s.resize(area, 0, Dir::Vertical, 6));
        assert_eq!(rects(&s, area).iter().map(|(_, r)| r.width).collect::<Vec<_>>(), [26, 14]);
        // The last pane grows by taking from the one before it.
        assert!(s.resize(area, 1, Dir::Vertical, 2));
        assert_eq!(rects(&s, area)[1].1.width, 16);
        // Nothing to resize vertically in a side-by-side split.
        assert!(!s.resize(area, 0, Dir::Horizontal, 2));
        // Shrinking stops at the minimum size.
        s.resize(area, 0, Dir::Vertical, -100);
        assert_eq!(rects(&s, area)[0].1.width, MIN_SIZE);
        s.equalize();
        assert_eq!(rects(&s, area).iter().map(|(_, r)| r.width).collect::<Vec<_>>(), [20, 20]);
    }

    #[test]
    fn resize_uses_the_nearest_split_in_that_direction() {
        let area = Rect::new(0, 0, 41, 21);
        let mut s = Split::Leaf(0);
        s.split(0, Dir::Vertical, 1);
        s.split(1, Dir::Horizontal, 2); // right column: 1 above 2
        assert!(s.resize(area, 2, Dir::Horizontal, 4));
        let r = rects(&s, area);
        assert_eq!((r[1].1.height, r[2].1.height), (6, 14));
        assert!(s.resize(area, 2, Dir::Vertical, 4), "width comes from the outer split");
        assert_eq!(rects(&s, area)[0].1.width, 16);
    }

    #[test]
    fn dragging_a_separator() {
        let area = Rect::new(0, 0, 41, 10);
        let mut s = Split::Leaf(0);
        s.split(0, Dir::Vertical, 1);
        let (mut panes, mut seps) = (Vec::new(), Vec::new());
        s.layout(area, &mut panes, &mut seps);
        s.drag(area, &seps[0], 10);
        let r = rects(&s, area);
        assert_eq!((r[0].1.width, r[1].1.x), (10, 11));
        s.drag(area, &seps[0], 0);
        assert_eq!(rects(&s, area)[0].1.width, MIN_SIZE, "clamped to the minimum");
    }

    #[test]
    fn layout_with_separators() {
        let s = Split::Node(Dir::Vertical, vec![(Split::Leaf(0), 1), (Split::Leaf(1), 1)]);
        let (mut panes, mut seps) = (Vec::new(), Vec::new());
        s.layout(Rect::new(0, 0, 21, 10), &mut panes, &mut seps);
        assert_eq!(panes, vec![(0, Rect::new(0, 0, 10, 10)), (1, Rect::new(11, 0, 10, 10))]);
        assert_eq!(seps.len(), 1);
        assert_eq!((seps[0].dir, seps[0].rect, seps[0].index), (Dir::Vertical, Rect::new(10, 0, 1, 10), 0));
        assert_eq!(neighbor(&panes, 0, 1, 0), Some(1));
        assert_eq!(neighbor(&panes, 1, -1, 0), Some(0));
        assert_eq!(neighbor(&panes, 0, 0, 1), None);
    }
}
