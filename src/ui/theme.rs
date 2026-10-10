//! Colors: Kakoune faces → ratatui styles, plus the chrome palette.

use ratatui::style::{Color, Modifier, Style};

use crate::kak::rpc::Face;

/// Palette for everything neokak draws itself (gruvbox dark, to match the
/// default Kakoune colorscheme pairing; editor text keeps Kakoune's faces).
#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    /// Floating windows. `Reset` keeps them on the terminal background,
    /// which suits transparent terminals.
    pub bg_float: Color,
    /// Tabline and statusline.
    pub bg_bar: Color,
    pub bg_alt: Color,
    pub bg_sel: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub border: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub purple: Color,
    pub aqua: Color,
    pub orange: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            bg_float: Color::Reset,
            bg_bar: rgb(0x1d2021),
            bg_alt: rgb(0x3c3836),
            bg_sel: rgb(0x504945),
            fg: rgb(0xebdbb2),
            fg_dim: rgb(0x928374),
            border: rgb(0x665c54),
            red: rgb(0xfb4934),
            green: rgb(0xb8bb26),
            yellow: rgb(0xfabd2f),
            blue: rgb(0x83a598),
            purple: rgb(0xd3869b),
            aqua: rgb(0x8ec07c),
            orange: rgb(0xfe8019),
        }
    }
}

impl Theme {
    pub fn float(&self) -> Style {
        Style::new().fg(self.fg).bg(self.bg_float)
    }

    pub fn border(&self) -> Style {
        Style::new().fg(self.border).bg(self.bg_float)
    }

    /// A Kakoune face carrying the chrome foreground, used as the base when
    /// resolving "default" colors inside floats.
    pub fn base_face(&self) -> Face {
        let hex = |c: Color| match c {
            Color::Rgb(r, g, b) => format!("rgb:{r:02x}{g:02x}{b:02x}"),
            _ => "default".into(),
        };
        Face { fg: hex(self.fg), bg: hex(self.bg_float), ..Face::default() }
    }

    /// Accent color for a Kakoune mode name (lualine's mode pill).
    pub fn mode_color(&self, mode: &str) -> Color {
        match mode {
            "normal" => self.blue,
            "insert" => self.green,
            "prompt" => self.yellow,
            m if m.starts_with("next-key") => self.purple,
            _ => self.orange,
        }
    }
}

fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn parse_color(s: &str) -> Option<Color> {
    let hex = s
        .strip_prefix("rgb:")
        .or_else(|| s.strip_prefix("rgba:").map(|h| h.get(..6).unwrap_or(h)))
        .or_else(|| s.strip_prefix('#'));
    if let Some(h) = hex {
        return u32::from_str_radix(h.get(..6)?, 16).ok().map(rgb);
    }
    Some(match s {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::Gray,
        "bright-black" => Color::DarkGray,
        "bright-red" => Color::LightRed,
        "bright-green" => Color::LightGreen,
        "bright-yellow" => Color::LightYellow,
        "bright-blue" => Color::LightBlue,
        "bright-magenta" => Color::LightMagenta,
        "bright-cyan" => Color::LightCyan,
        "bright-white" => Color::White,
        _ => return None,
    })
}

/// Resolves `face` against `base` (Kakoune's "default" means "inherit").
pub fn face_style(face: &Face, base: &Face) -> Style {
    let pick = |c: &str, b: &str| {
        let c = if c.is_empty() || c == "default" { b } else { c };
        parse_color(c).unwrap_or(Color::Reset)
    };
    let mut fg = pick(&face.fg, &base.fg);
    let mut bg = pick(&face.bg, &base.bg);
    let mut style = Style::new();
    let attrs = face.attributes.iter().chain(base.attributes.iter().filter(|_| face.attributes.is_empty()));
    for attr in attrs {
        style = match attr.as_str() {
            "bold" => style.add_modifier(Modifier::BOLD),
            "dim" => style.add_modifier(Modifier::DIM),
            "italic" => style.add_modifier(Modifier::ITALIC),
            "blink" => style.add_modifier(Modifier::SLOW_BLINK),
            "strikethrough" => style.add_modifier(Modifier::CROSSED_OUT),
            "underline" | "curly_underline" | "double_underline" => style.add_modifier(Modifier::UNDERLINED),
            "reverse" => {
                std::mem::swap(&mut fg, &mut bg);
                // Reversing onto the terminal default needs real colors.
                if fg == Color::Reset {
                    fg = Color::Black;
                }
                if bg == Color::Reset {
                    bg = Color::Gray;
                }
                style
            }
            _ => style,
        };
    }
    style.fg(fg).bg(bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(fg: &str, bg: &str, attrs: &[&str]) -> Face {
        Face {
            fg: fg.into(),
            bg: bg.into(),
            underline: String::new(),
            attributes: attrs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("rgb:ebdbb2"), Some(Color::Rgb(0xeb, 0xdb, 0xb2)));
        assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("rgba:11223380"), Some(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(parse_color("bright-black"), Some(Color::DarkGray));
        assert_eq!(parse_color("default"), None);
    }

    #[test]
    fn default_inherits_base() {
        let base = face("rgb:ebdbb2", "default", &[]);
        let s = face_style(&face("default", "red", &["bold"]), &base);
        assert_eq!(s.fg, Some(Color::Rgb(0xeb, 0xdb, 0xb2)));
        assert_eq!(s.bg, Some(Color::Red));
        assert!(s.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn reverse_swaps() {
        let s = face_style(&face("red", "blue", &["reverse"]), &Face::default());
        assert_eq!((s.fg, s.bg), (Some(Color::Blue), Some(Color::Red)));
    }
}
