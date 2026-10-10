//! Nerd Font glyphs, with a plain fallback for terminals without one.

#[derive(Debug, Clone, Copy)]
pub struct Icons {
    pub nerd: bool,
}

impl Icons {
    fn pick(&self, nerd: &'static str, plain: &'static str) -> &'static str {
        if self.nerd { nerd } else { plain }
    }

    pub fn sep_left(&self) -> &'static str {
        self.pick("\u{e0b4}", "")
    }
    pub fn sep_right(&self) -> &'static str {
        self.pick("\u{e0b6}", "")
    }
    pub fn branch(&self) -> &'static str {
        self.pick("\u{e725} ", "git:")
    }
    pub fn modified(&self) -> &'static str {
        self.pick("\u{25cf}", "+")
    }
    pub fn error(&self) -> &'static str {
        self.pick("\u{f057} ", "E:")
    }
    pub fn warning(&self) -> &'static str {
        self.pick("\u{f071} ", "W:")
    }
    pub fn prompt(&self) -> &'static str {
        self.pick("\u{f054} ", "> ")
    }
    pub fn cmdline(&self) -> &'static str {
        self.pick("\u{f120} ", "")
    }
    pub fn search(&self) -> &'static str {
        self.pick("\u{f002} ", "")
    }
    pub fn arrow(&self) -> &'static str {
        self.pick("\u{279c}", "->")
    }
    pub fn folder(&self, open: bool) -> &'static str {
        match (self.nerd, open) {
            (true, true) => "\u{f07c} ",
            (true, false) => "\u{f07b} ",
            (false, true) => "v ",
            (false, false) => "> ",
        }
    }
    pub fn tab_active(&self) -> &'static str {
        "\u{258e}"
    }

    pub fn level(&self, level: &str) -> &'static str {
        match level {
            "error" => self.pick("\u{f057} ", "[error] "),
            "warn" => self.pick("\u{f071} ", "[warn] "),
            _ => self.pick("\u{f05a} ", "[info] "),
        }
    }

    /// File-type icon for a path, chosen by name and extension.
    pub fn file(&self, path: &str) -> &'static str {
        if !self.nerd {
            return "";
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        if name.starts_with('*') {
            return "\u{f489} ";
        }
        match name {
            "Cargo.toml" | "Cargo.lock" => return "\u{e7a8} ",
            "Makefile" | "makefile" => return "\u{e779} ",
            "Dockerfile" => return "\u{f308} ",
            ".gitignore" | ".gitmodules" => return "\u{e702} ",
            "kakrc" => return "\u{f121} ",
            _ => {}
        }
        let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        match ext {
            "rs" => "\u{e7a8} ",
            "py" => "\u{e73c} ",
            "js" | "mjs" | "cjs" => "\u{e74e} ",
            "ts" | "tsx" => "\u{e628} ",
            "jsx" => "\u{e7ba} ",
            "go" => "\u{e627} ",
            "c" | "h" => "\u{e61e} ",
            "cpp" | "cc" | "hpp" | "cxx" => "\u{e61d} ",
            "java" => "\u{e738} ",
            "lua" => "\u{e620} ",
            "rb" => "\u{e739} ",
            "sh" | "bash" | "zsh" | "fish" => "\u{f489} ",
            "md" | "markdown" => "\u{e73e} ",
            "json" => "\u{e60b} ",
            "toml" | "yaml" | "yml" | "ini" | "conf" => "\u{e615} ",
            "html" | "htm" => "\u{e736} ",
            "css" | "scss" => "\u{e749} ",
            "nix" => "\u{f313} ",
            "kak" => "\u{f121} ",
            "lock" => "\u{f023} ",
            "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" => "\u{f1c5} ",
            "txt" => "\u{f15c} ",
            _ => "\u{f15b} ",
        }
    }
}
