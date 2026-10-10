//! Colors: Kakoune faces → ratatui styles, plus the chrome palette.

use ratatui::style::{Color, Modifier, Style};

use crate::kak::rpc::Face;

/// Palette for everything neokak draws itself. Text inside the editor keeps
/// the Kakoune colorscheme's faces.
#[derive(Debug, Clone, PartialEq)]
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
    /// Light text on dark backgrounds (affects contrast fixes).
    pub dark: bool,
}

/// Built-in theme names, in picker order.
pub const THEMES: &[&str] = &["gruvbox", "gruvbox-light", "tokyonight", "catppuccin", "nord", "onedark", "kakoune"];

/// Colors a user can override, as `name=color`.
pub const COLOR_NAMES: &[&str] = &[
    "bg_float", "bg_bar", "bg_alt", "bg_sel", "fg", "fg_dim", "border", "red", "green", "yellow", "blue", "purple",
    "aqua", "orange",
];

impl Default for Theme {
    fn default() -> Self {
        Self::named("gruvbox").unwrap().transparent(true)
    }
}

/// float, bar, alt, sel, fg, fg_dim, border, red, green, yellow, blue,
/// purple, aqua, orange
type Palette = [u32; 14];

fn palette(name: &str) -> Option<(Palette, bool)> {
    Some(match name {
        "gruvbox" => (
            [
                0x1d2021, 0x1d2021, 0x3c3836, 0x504945, 0xebdbb2, 0x928374, 0x665c54, 0xfb4934, 0xb8bb26, 0xfabd2f,
                0x83a598, 0xd3869b, 0x8ec07c, 0xfe8019,
            ],
            true,
        ),
        "gruvbox-light" => (
            [
                0xf9f5d7, 0xf2e5bc, 0xebdbb2, 0xd5c4a1, 0x3c3836, 0x928374, 0xbdae93, 0x9d0006, 0x79740e, 0xb57614,
                0x076678, 0x8f3f71, 0x427b58, 0xaf3a03,
            ],
            false,
        ),
        "tokyonight" => (
            [
                0x16161e, 0x16161e, 0x292e42, 0x283457, 0xc0caf5, 0x565f89, 0x3b4261, 0xf7768e, 0x9ece6a, 0xe0af68,
                0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xff9e64,
            ],
            true,
        ),
        "catppuccin" => (
            [
                0x181825, 0x181825, 0x313244, 0x45475a, 0xcdd6f4, 0x7f849c, 0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af,
                0x89b4fa, 0xcba6f7, 0x94e2d5, 0xfab387,
            ],
            true,
        ),
        "nord" => (
            [
                0x2e3440, 0x2e3440, 0x3b4252, 0x434c5e, 0xd8dee9, 0x616e88, 0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b,
                0x81a1c1, 0xb48ead, 0x88c0d0, 0xd08770,
            ],
            true,
        ),
        "onedark" => (
            [
                0x21252b, 0x21252b, 0x2c313c, 0x3e4451, 0xabb2bf, 0x5c6370, 0x3e4452, 0xe06c75, 0x98c379, 0xe5c07b,
                0x61afef, 0xc678dd, 0x56b6c2, 0xd19a66,
            ],
            true,
        ),
        _ => return None,
    })
}

impl Theme {
    /// A built-in theme, with solid popup backgrounds.
    pub fn named(name: &str) -> Option<Self> {
        let ([float, bar, alt, sel, fg, fg_dim, border, red, green, yellow, blue, purple, aqua, orange], dark) =
            palette(name)?;
        Some(Self {
            bg: Color::Reset,
            bg_float: rgb(float),
            bg_bar: rgb(bar),
            bg_alt: rgb(alt),
            bg_sel: rgb(sel),
            fg: rgb(fg),
            fg_dim: rgb(fg_dim),
            border: rgb(border),
            red: rgb(red),
            green: rgb(green),
            yellow: rgb(yellow),
            blue: rgb(blue),
            purple: rgb(purple),
            aqua: rgb(aqua),
            orange: rgb(orange),
            dark,
        })
    }

    /// A theme following the Kakoune colorscheme, from the faces in the
    /// modeline probe. Faces the colorscheme leaves at "default" keep the
    /// gruvbox color.
    pub fn from_kakoune(probe: &[(char, Face)]) -> Self {
        let face = |c: char| probe.iter().find(|(k, _)| *k == c).map(|(_, f)| f);
        let color = |s: &str| (s != "default" && !s.is_empty()).then(|| parse_color(s)).flatten();
        let fg = |c| face(c).and_then(|f| color(&f.fg));
        let bg = |c| face(c).and_then(|f| color(&f.bg));
        let mut t = Self::named("gruvbox").unwrap();
        let set = |slot: &mut Color, value: Option<Color>| {
            if let Some(v) = value {
                *slot = v;
            }
        };
        set(&mut t.fg, fg('D').or(fg('S')));
        set(&mut t.bg_bar, bg('S'));
        set(&mut t.bg_float, bg('M'));
        set(&mut t.bg_alt, bg('M').or(bg('S')));
        set(&mut t.bg_sel, bg('F'));
        set(&mut t.fg_dim, fg('c').or(fg('L')));
        set(&mut t.border, fg('L').or(fg('P')).or(fg('c')));
        set(&mut t.blue, fg('f'));
        set(&mut t.purple, fg('k'));
        set(&mut t.green, fg('s'));
        set(&mut t.yellow, fg('t'));
        set(&mut t.orange, fg('v'));
        set(&mut t.aqua, fg('m'));
        // Error is often black on red; take whichever side is the color.
        let error = face('E').and_then(|f| match (color(&f.fg), color(&f.bg)) {
            (Some(Color::Black) | None, Some(b)) => Some(b),
            (f, _) => f,
        });
        set(&mut t.red, error);
        t.dark = match t.fg {
            Color::Rgb(r, g, b) => 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32 > 128.0,
            Color::Black | Color::DarkGray => false,
            _ => true,
        };
        t
    }

    /// Draws popups on the terminal background instead of a solid color.
    pub fn transparent(mut self, on: bool) -> Self {
        if on {
            self.bg_float = Color::Reset;
        }
        self
    }

    /// Overrides one color, e.g. `set_color("blue", "#7aa2f7")`.
    pub fn set_color(&mut self, name: &str, value: &str) -> Result<(), String> {
        let color = parse_color(value).ok_or_else(|| format!("bad color {value:?} for {name}"))?;
        let slot = match name {
            "bg_float" => &mut self.bg_float,
            "bg_bar" => &mut self.bg_bar,
            "bg_alt" => &mut self.bg_alt,
            "bg_sel" => &mut self.bg_sel,
            "fg" => &mut self.fg,
            "fg_dim" => &mut self.fg_dim,
            "border" => &mut self.border,
            "red" => &mut self.red,
            "green" => &mut self.green,
            "yellow" => &mut self.yellow,
            "blue" => &mut self.blue,
            "purple" => &mut self.purple,
            "aqua" => &mut self.aqua,
            "orange" => &mut self.orange,
            _ => return Err(format!("unknown theme color {name:?} (one of: {})", COLOR_NAMES.join(", "))),
        };
        *slot = color;
        Ok(())
    }

    /// Builds a theme from neokak's options: the theme name, transparency,
    /// and `name=color` overrides. Errors name every bad setting; the theme
    /// is still built from the valid ones.
    pub fn from_settings(
        name: &str,
        transparent: bool,
        overrides: &[String],
        probe: &[(char, Face)],
    ) -> (Self, Vec<String>) {
        let mut errors = Vec::new();
        let base = match name {
            "kakoune" => Some(Self::from_kakoune(probe)),
            _ => Self::named(name),
        };
        let base = base.unwrap_or_else(|| {
            errors.push(format!("unknown theme {name:?} (one of: {})", THEMES.join(", ")));
            Self::named("gruvbox").unwrap()
        });
        let mut theme = base.transparent(transparent);
        for o in overrides.iter().filter(|o| !o.is_empty()) {
            let res = match o.split_once('=') {
                Some((k, v)) => theme.set_color(k.trim(), v.trim()),
                None => Err(format!("theme override {o:?} should look like name=color")),
            };
            if let Err(e) = res {
                errors.push(e);
            }
        }
        (theme, errors)
    }

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
    fn every_theme_builds() {
        for name in THEMES.iter().filter(|n| **n != "kakoune") {
            let t = Theme::named(name).unwrap_or_else(|| panic!("{name} missing"));
            assert_ne!(t.bg_float, Color::Reset, "{name}: solid popups unless transparent");
            assert_eq!(t.clone().transparent(true).bg_float, Color::Reset);
        }
        assert!(!Theme::named("gruvbox-light").unwrap().dark);
        assert!(Theme::named("nope").is_none());
    }

    #[test]
    fn settings_apply_overrides_and_report_errors() {
        let overrides = ["blue=#7aa2f7", "bg_sel=rgb:3b4261", "pink=#ff00ff", "fg=notacolor", "junk"].map(String::from);
        let (t, errors) = Theme::from_settings("tokyonight", false, &overrides, &[]);
        assert_eq!(t.blue, Color::Rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(t.bg_sel, Color::Rgb(0x3b, 0x42, 0x61));
        assert_eq!(t.bg_float, Theme::named("tokyonight").unwrap().bg_float, "not transparent");
        assert_eq!(errors.len(), 3, "{errors:?}");

        let (t, errors) = Theme::from_settings("nope", true, &[], &[]);
        assert_eq!(t, Theme::default(), "unknown names fall back to transparent gruvbox");
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn kakoune_theme_follows_the_probe() {
        let probe = [
            ('f', face("rgb:112233", "default", &[])),
            ('S', face("rgb:eeeeee", "rgb:202020", &[])),
            ('E', face("black", "red", &[])),
            ('k', face("default", "default", &[])),
        ];
        let t = Theme::from_kakoune(&probe);
        assert_eq!(t.blue, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(t.bg_bar, Color::Rgb(0x20, 0x20, 0x20));
        assert_eq!(t.fg, Color::Rgb(0xee, 0xee, 0xee));
        assert_eq!(t.red, Color::Red, "takes the colored side of black-on-red");
        assert_eq!(t.purple, Theme::named("gruvbox").unwrap().purple, "default faces keep gruvbox");
        assert!(t.dark);
        let (from_settings, errors) = Theme::from_settings("kakoune", false, &[], &probe);
        assert!(errors.is_empty());
        assert_eq!(from_settings, t);
    }

    #[test]
    fn rc_completions_list_every_theme() {
        let line = crate::RC.lines().find(|l| l.contains("printf '%s\\n' gruvbox")).expect("neokak-theme candidates");
        let listed: Vec<&str> = line.split_whitespace().skip(2).collect();
        assert_eq!(listed, THEMES, "keep neokak-theme's completions in rc/neokak.kak in sync");
        for name in COLOR_NAMES {
            assert!(crate::RC.contains(name), "rc docstring should mention {name}");
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
