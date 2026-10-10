//! Lualine-style statusline and the message line under it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{Seg, atom_segs, fill, put, put_segs, segs_width};
use super::icons::Icons;
use super::theme::Theme;
use crate::kak::rpc::{Face, Line, line_text};

/// Field separator neokak's `modelinefmt` uses (see rc/neokak.kak).
pub const SEP: char = '\u{241e}';

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ModeLine {
    pub mode: String,
    pub bufname: String,
    pub modified: bool,
    pub line: String,
    pub column: String,
    pub line_count: String,
    pub filetype: String,
    pub mode_info: String,
    pub errors: u32,
    pub warnings: u32,
}

impl ModeLine {
    pub fn parse(line: &Line) -> Option<Self> {
        let text = line_text(line);
        let f: Vec<&str> = text.split(SEP).map(str::trim).collect();
        if f.len() < 10 {
            return None;
        }
        Some(Self {
            mode: f[0].to_owned(),
            bufname: f[1].to_owned(),
            modified: f[2] == "true",
            line: f[3].to_owned(),
            column: f[4].to_owned(),
            line_count: f[5].to_owned(),
            filetype: f[6].to_owned(),
            mode_info: f[7].to_owned(),
            errors: f[8].parse().unwrap_or(0),
            warnings: f[9].parse().unwrap_or(0),
        })
    }

    fn progress(&self) -> String {
        match (self.line.parse::<u64>(), self.line_count.parse::<u64>()) {
            (Ok(1), _) => "Top".into(),
            (Ok(l), Ok(n)) if l >= n => "Bot".into(),
            (Ok(l), Ok(n)) if n > 0 => format!("{}%", l * 100 / n),
            _ => String::new(),
        }
    }
}

/// The colorscheme probe at the end of neokak's modeline: one single-char
/// atom per face, in the face's resolved colors (see rc/neokak.kak).
pub fn face_probe(line: &Line) -> Vec<(char, Face)> {
    let mut seps = 0;
    let mut out = Vec::new();
    for atom in line {
        if seps >= 10 {
            let mut chars = atom.contents.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                out.push((c, atom.face.clone()));
            }
        }
        seps += atom.contents.matches(SEP).count();
    }
    out
}

pub fn mode_label(mode: &str) -> String {
    match mode {
        "prompt" => "COMMAND".into(),
        m if m.starts_with("next-key") => {
            // e.g. next-key[user-mapping], next-key[user.find]
            let sub = m.trim_start_matches("next-key").trim_matches(['[', ']']);
            match sub.rsplit('.').next().unwrap_or(sub) {
                "" => "PENDING".into(),
                "user-mapping" => "USER".into(),
                s => s.to_uppercase(),
            }
        }
        m => m.to_uppercase(),
    }
}

pub struct StatusContext<'a> {
    pub mode_line: &'a Line,
    pub branch: Option<&'a str>,
    /// What has focus when it isn't the editor (`EXPLORER`, `TERMINAL`);
    /// replaces the mode in the pill.
    pub focus: Option<&'a str>,
}

pub fn render(buf: &mut Buffer, area: Rect, ctx: &StatusContext, theme: &Theme, icons: &Icons) {
    let mid = Style::new().fg(theme.fg).bg(theme.bg_bar);
    fill(buf, area, mid);

    let Some(ml) = ModeLine::parse(ctx.mode_line) else {
        // Someone overrode modelinefmt; show it verbatim.
        put_segs(buf, area.x, area.y, area.right(), &atom_segs(ctx.mode_line, &Face::default(), Some(theme.bg_bar)));
        return;
    };

    let (label, accent) = match ctx.focus {
        Some(what) => (what.to_uppercase(), theme.aqua),
        None => (mode_label(&ml.mode), theme.mode_color(&ml.mode)),
    };
    let a = Style::new().fg(theme.bg_bar).bg(accent).add_modifier(Modifier::BOLD);
    let b = Style::new().fg(theme.fg).bg(theme.bg_alt);
    let sep_a = Style::new().fg(accent).bg(theme.bg_alt);
    let sep_b = Style::new().fg(theme.bg_alt).bg(theme.bg_bar);

    let mut left: Vec<Seg> = vec![(format!(" {label} "), a), (icons.sep_left().into(), sep_a)];
    match ctx.branch {
        Some(branch) => left.push((format!(" {}{branch} ", icons.branch()), b)),
        None => left.push((" ".into(), b)),
    }
    left.push((icons.sep_left().into(), sep_b));
    let name = Style::new().fg(theme.fg).bg(theme.bg_bar).add_modifier(Modifier::BOLD);
    left.push((format!(" {}{}", icons.file(&ml.bufname), ml.bufname), name));
    if ml.modified {
        left.push((format!(" {}", icons.modified()), mid.fg(theme.orange)));
    }
    if ml.errors > 0 {
        left.push((format!("  {}{}", icons.error(), ml.errors), mid.fg(theme.red)));
    }
    if ml.warnings > 0 {
        left.push((format!("  {}{}", icons.warning(), ml.warnings), mid.fg(theme.yellow)));
    }

    let mut right: Vec<Seg> = Vec::new();
    // In normal mode, mode_info carries the selection count plus any pending
    // count/register; elsewhere it just repeats the mode name.
    if ml.mode == "normal" && !ml.mode_info.is_empty() && ml.mode_info != "1 sel" {
        right.push((format!("{}  ", ml.mode_info), mid.fg(theme.aqua)));
    }
    if !ml.filetype.is_empty() {
        right.push((
            format!("{}{}  ", icons.file(&format!("x.{}", ft_ext(&ml.filetype))), ml.filetype),
            mid.fg(theme.fg_dim),
        ));
    }
    right.push((icons.sep_right().into(), sep_b));
    right.push((format!(" {} ", ml.progress()), b));
    right.push((icons.sep_right().into(), sep_a));
    right.push((format!(" {}:{} ", ml.line, ml.column), a));

    let right_w = segs_width(&right) as u16;
    let right_x = area.right().saturating_sub(right_w).max(area.x);
    put_segs(buf, area.x, area.y, right_x, &left);
    put_segs(buf, right_x, area.y, area.right(), &right);
}

/// Maps a Kakoune filetype to an extension so it gets the matching icon.
fn ft_ext(ft: &str) -> &str {
    match ft {
        "rust" => "rs",
        "python" => "py",
        "javascript" => "js",
        "typescript" => "ts",
        "markdown" => "md",
        "kak" => "kak",
        other => other,
    }
}

/// The line under the statusline: echo messages and errors.
pub fn render_messages(buf: &mut Buffer, area: Rect, content: &Line, default_face: &Face) {
    fill(buf, area, Style::new());
    let x = put(buf, area.x, area.y, area.right(), " ", Style::new());
    put_segs(buf, x, area.y, area.right(), &atom_segs(content, default_face, None));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kak::rpc::Atom;

    fn line(s: &str) -> Line {
        vec![Atom { face: Face::default(), contents: s.into() }]
    }

    #[test]
    fn parses_modeline() {
        let ml = ModeLine::parse(&line("insert␞main.rs␞true␞10␞4␞200␞rust␞insert␞1␞3")).unwrap();
        assert_eq!(ml.mode, "insert");
        assert!(ml.modified);
        assert_eq!((ml.errors, ml.warnings), (1, 3));
        assert_eq!(ml.progress(), "5%");
        assert!(ModeLine::parse(&line("t.txt 1:1")).is_none());
    }

    fn render_text(mode_line: &Line, branch: Option<&str>) -> String {
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        let ctx = StatusContext { mode_line, branch, focus: None };
        render(&mut buf, area, &ctx, &Theme::default(), &Icons { nerd: false });
        crate::ui::draw::rows(&buf).join("")
    }

    #[test]
    fn renders_lualine_sections() {
        let text = render_text(&line("normal␞src/main.rs␞true␞12␞4␞100␞rust␞3 sels␞2␞1"), Some("main"));
        for part in ["NORMAL", "git:main", "src/main.rs", "+", "E:2", "W:1", "3 sels", "rust", "12%", "12:4"] {
            assert!(text.contains(part), "missing {part:?} in {text:?}");
        }
    }

    #[test]
    fn hides_redundant_mode_info() {
        let text = render_text(&line("insert␞a.txt␞false␞1␞1␞1␞␞insert␞0␞0"), None);
        assert!(text.contains("INSERT"));
        assert_eq!(text.matches("insert").count(), 0, "{text:?}");
        assert!(!text.contains("E:") && !text.contains("git:"));
    }

    #[test]
    fn shows_what_has_focus() {
        let ml = line("normal␞a.txt␞false␞1␞1␞1␞␞1 sel␞0␞0");
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        let ctx = StatusContext { mode_line: &ml, branch: None, focus: Some("explorer") };
        render(&mut buf, area, &ctx, &Theme::default(), &Icons { nerd: false });
        let text = crate::ui::draw::rows(&buf).join("");
        assert!(text.contains(" EXPLORER ") && !text.contains("NORMAL"), "{text}");
    }

    #[test]
    fn falls_back_to_raw_modeline() {
        let text = render_text(&line("custom modeline 1:1"), None);
        assert!(text.contains("custom modeline 1:1"));
    }

    #[test]
    fn reads_the_face_probe() {
        let atom = |s: &str, fg: &str| Atom { face: Face { fg: fg.into(), ..Face::default() }, contents: s.into() };
        let line = vec![
            atom("normal␞a␞false␞1␞1␞1␞rust␞1 sel␞0␞0␞", "default"),
            atom("f", "rgb:83a598"),
            atom("c", "rgb:928374"),
        ];
        let probe = face_probe(&line);
        assert_eq!(probe.len(), 2);
        assert_eq!((probe[0].0, probe[0].1.fg.as_str()), ('f', "rgb:83a598"));
        assert!(face_probe(&self::line("normal␞a")).is_empty());
        assert!(ModeLine::parse(&line).is_some(), "the probe doesn't break field parsing");
    }

    #[test]
    fn labels() {
        assert_eq!(mode_label("normal"), "NORMAL");
        assert_eq!(mode_label("prompt"), "COMMAND");
        assert_eq!(mode_label("next-key[user-mapping]"), "USER");
        assert_eq!(mode_label("next-key[user.find]"), "FIND");
        assert_eq!(mode_label("next-key"), "PENDING");
    }
}
