//! `config.toml`: neokak's settings and leader keys.
//!
//! Each setting maps to a Kakoune option (or command). neokak turns the file
//! into a kakscript that `rc/neokak.kak` sources before the user's
//! `init.kak`, and re-applies it whenever the file changes. The settings
//! screen (`<space>o`) edits the same file, keeping its comments.
//!
//! The leader keymap lives here too: the built-in bindings below, with the
//! file's `[keys]` table layered on top.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table, Value};

use crate::kak::session::quote;

/// The kind of value a setting takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    Str,
    /// One of a fixed set of strings.
    Choice(&'static [&'static str]),
    /// A list of strings.
    List,
}

/// What a setting changes in Kakoune.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Option(&'static str),
    /// `colorscheme <value>`.
    Colorscheme,
}

#[derive(Debug, Clone, Copy)]
pub struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub kind: Kind,
    pub target: Target,
    /// The default, as a TOML value.
    pub default: &'static str,
    pub doc: &'static str,
}

impl Setting {
    pub fn path(&self) -> String {
        format!("{}.{}", self.section, self.key)
    }

    pub fn default_value(&self) -> Value {
        self.default.parse().expect("valid default")
    }

    /// Checks a value from the file.
    fn check(&self, v: &Value) -> Result<(), String> {
        let ok = match (self.kind, v) {
            (Kind::Bool, Value::Boolean(_)) | (Kind::Int, Value::Integer(_)) | (Kind::Str, Value::String(_)) => true,
            (Kind::Choice(choices), Value::String(s)) => {
                if !choices.contains(&s.value().as_str()) {
                    return Err(format!("{}: expected one of {}", self.path(), choices.join(", ")));
                }
                true
            }
            (Kind::List, Value::Array(a)) => a.iter().all(|e| e.is_str()),
            _ => false,
        };
        let expected = match self.kind {
            Kind::Bool => "true or false",
            Kind::Int => "a number",
            Kind::Str | Kind::Choice(_) => "a string",
            Kind::List => "a list of strings",
        };
        if ok { Ok(()) } else { Err(format!("{}: expected {expected}", self.path())) }
    }

    /// The Kakoune command that applies `v`.
    fn command(&self, v: &Value) -> String {
        match self.target {
            Target::Colorscheme => {
                let name = v.as_str().filter(|s| !s.is_empty()).unwrap_or("default");
                format!("colorscheme {}", quote(name))
            }
            Target::Option(opt) => format!("set-option global {opt} {}", kak_words(v)),
        }
    }
}

/// A value as Kakoune option words.
fn kak_words(v: &Value) -> String {
    match v {
        Value::Boolean(b) => b.value().to_string(),
        Value::Integer(i) => i.value().to_string(),
        Value::String(s) => quote(s.value()),
        Value::Array(a) => a.iter().filter_map(|e| e.as_str()).map(quote).collect::<Vec<_>>().join(" "),
        other => quote(&other.to_string()),
    }
}

/// A value as shown on the settings screen.
pub fn display(v: &Value) -> String {
    match v {
        Value::String(s) if s.value().is_empty() => "(empty)".into(),
        Value::String(s) => s.value().clone(),
        Value::Array(a) if a.is_empty() => "(none)".into(),
        Value::Array(a) => format!("{} entr{}", a.len(), if a.len() == 1 { "y" } else { "ies" }),
        other => other.to_string().trim().to_owned(),
    }
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        section: "terminal",
        key: "shell",
        kind: Kind::Str,
        target: Target::Option("neokak_terminal_shell"),
        default: r#""""#,
        doc: "Shell for the floating terminal (empty: $SHELL)",
    },
    Setting {
        section: "theme",
        key: "name",
        kind: Kind::Choice(crate::ui::theme::THEMES),
        target: Target::Option("neokak_theme"),
        default: r#""gruvbox""#,
        doc: "Interface theme (kakoune follows the colorscheme)",
    },
    Setting {
        section: "theme",
        key: "transparent",
        kind: Kind::Bool,
        target: Target::Option("neokak_transparent"),
        default: "true",
        doc: "Draw popups on the terminal background",
    },
    Setting {
        section: "theme",
        key: "colors",
        kind: Kind::List,
        target: Target::Option("neokak_theme_colors"),
        default: "[]",
        doc: "Theme color overrides, e.g. \"blue=#7aa2f7\"",
    },
    Setting {
        section: "editor",
        key: "colorscheme",
        kind: Kind::Str,
        target: Target::Colorscheme,
        default: r#""""#,
        doc: "Kakoune colorscheme (empty: default)",
    },
    Setting {
        section: "editor",
        key: "tabstop",
        kind: Kind::Int,
        target: Target::Option("tabstop"),
        default: "8",
        doc: "Width of a tab character",
    },
    Setting {
        section: "editor",
        key: "indentwidth",
        kind: Kind::Int,
        target: Target::Option("indentwidth"),
        default: "4",
        doc: "Spaces per indent level (0: indent with tabs)",
    },
    Setting {
        section: "git",
        key: "signs",
        kind: Kind::Bool,
        target: Target::Option("neokak_git_signs"),
        default: "true",
        doc: "Git change markers in the gutter",
    },
    Setting {
        section: "git",
        key: "blame_line",
        kind: Kind::Bool,
        target: Target::Option("neokak_git_blame_line"),
        default: "false",
        doc: "Show who last changed the cursor line",
    },
    Setting {
        section: "git",
        key: "tool",
        kind: Kind::Str,
        target: Target::Option("neokak_git_tool"),
        default: r#""lazygit""#,
        doc: "Program <space>gg runs",
    },
    Setting {
        section: "session",
        key: "save",
        kind: Kind::Bool,
        target: Target::Option("neokak_session"),
        default: "true",
        doc: "Save the open files of each project",
    },
    Setting {
        section: "session",
        key: "autorestore",
        kind: Kind::Bool,
        target: Target::Option("neokak_session_autorestore"),
        default: "false",
        doc: "Restore the session when started without files",
    },
    Setting {
        section: "lsp",
        key: "pickers",
        kind: Kind::Bool,
        target: Target::Option("neokak_lsp_pickers"),
        default: "true",
        doc: "Show kak-lsp result lists in pickers",
    },
    Setting {
        section: "lsp",
        key: "hide_hints_in_insert",
        kind: Kind::Bool,
        target: Target::Option("neokak_hide_hints_in_insert"),
        default: "true",
        doc: "Hide inlay hints in insert mode",
    },
    Setting {
        section: "treesitter",
        key: "faces",
        kind: Kind::Bool,
        target: Target::Option("neokak_ts_faces"),
        default: "true",
        doc: "Link tree-sitter faces to the colorscheme's",
    },
    Setting {
        section: "env",
        key: "providers",
        kind: Kind::List,
        target: Target::Option("neokak_env_providers"),
        default: r#"[
    '.envrc:if command -v direnv >/dev/null; then direnv exec . env -0; fi',
    'devenv.nix:if command -v devenv >/dev/null; then devenv hook-should-activate >/dev/null && devenv shell -- env -0; fi',
]"#,
        doc: "<marker>:<command> pairs that load project environments",
    },
];

/// A leader binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Kakoune user mode: `user` for the leader itself.
    pub mode: String,
    pub key: String,
    /// The keys the binding types (`: command<ret>`).
    pub keys: String,
    pub doc: String,
}

struct Builtin {
    mode: &'static str,
    key: &'static str,
    keys: &'static str,
    doc: &'static str,
}

/// The built-in leader keymap.
const BUILTIN_KEYS: &[Builtin] = &[
    Builtin { mode: "user", key: "<space>", keys: ": neokak-picker files<ret>", doc: "find files" },
    Builtin { mode: "user", key: ",", keys: ": neokak-picker buffers<ret>", doc: "switch buffer" },
    Builtin { mode: "user", key: "e", keys: ": neokak-explorer<ret>", doc: "file explorer" },
    Builtin { mode: "user", key: "t", keys: ": neokak-msg terminal<ret>", doc: "terminal (<c-\\> hides)" },
    Builtin { mode: "user", key: "o", keys: ": neokak-settings<ret>", doc: "settings" },
    Builtin { mode: "user", key: "f", keys: ": enter-user-mode find<ret>", doc: "+find" },
    Builtin { mode: "user", key: "b", keys: ": enter-user-mode buffer<ret>", doc: "+buffer" },
    Builtin { mode: "user", key: "w", keys: ": enter-user-mode window<ret>", doc: "+window" },
    Builtin { mode: "user", key: "l", keys: ": enter-user-mode lsp<ret>", doc: "+lsp" },
    Builtin { mode: "user", key: "s", keys: ": enter-user-mode surround<ret>", doc: "+surround" },
    Builtin { mode: "user", key: "g", keys: ": enter-user-mode git<ret>", doc: "+git" },
    Builtin { mode: "user", key: "q", keys: ": enter-user-mode session<ret>", doc: "+session" },
    Builtin { mode: "user", key: "z", keys: ": enter-user-mode fold<ret>", doc: "+fold" },
    Builtin { mode: "user", key: "j", keys: ": neokak-goto-symbol next function<ret>", doc: "next function" },
    Builtin { mode: "user", key: "k", keys: ": neokak-goto-symbol previous function<ret>", doc: "previous function" },
    Builtin { mode: "user", key: "J", keys: ": neokak-goto-symbol next symbol<ret>", doc: "next symbol" },
    Builtin { mode: "user", key: "K", keys: ": neokak-goto-symbol previous symbol<ret>", doc: "previous symbol" },
    Builtin { mode: "user", key: "|", keys: ": neokak-window vsplit<ret>", doc: "split right" },
    Builtin { mode: "user", key: "<minus>", keys: ": neokak-window split<ret>", doc: "split below" },
    Builtin { mode: "find", key: "f", keys: ": neokak-picker files<ret>", doc: "files" },
    Builtin { mode: "find", key: "g", keys: ": neokak-picker grep<ret>", doc: "live grep" },
    Builtin { mode: "find", key: "b", keys: ": neokak-picker buffers<ret>", doc: "buffers" },
    Builtin { mode: "find", key: "r", keys: ": neokak-picker recent<ret>", doc: "recent files" },
    Builtin { mode: "find", key: "c", keys: ": neokak-picker colorschemes<ret>", doc: "colorschemes" },
    Builtin { mode: "find", key: "t", keys: ": neokak-picker themes<ret>", doc: "interface themes" },
    Builtin { mode: "find", key: "p", keys: ": neokak-picker pickers<ret>", doc: "all pickers" },
    Builtin { mode: "buffer", key: "b", keys: ": neokak-picker buffers<ret>", doc: "pick buffer" },
    Builtin { mode: "buffer", key: "n", keys: ": buffer-next<ret>", doc: "next buffer" },
    Builtin { mode: "buffer", key: "p", keys: ": buffer-previous<ret>", doc: "previous buffer" },
    Builtin { mode: "buffer", key: "d", keys: ": delete-buffer<ret>", doc: "delete buffer" },
    Builtin { mode: "buffer", key: "D", keys: ": delete-buffer!<ret>", doc: "delete buffer (discard changes)" },
    Builtin { mode: "buffer", key: "1", keys: ": neokak-buffer-nth 1<ret>", doc: "buffer 1" },
    Builtin { mode: "buffer", key: "2", keys: ": neokak-buffer-nth 2<ret>", doc: "buffer 2" },
    Builtin { mode: "buffer", key: "3", keys: ": neokak-buffer-nth 3<ret>", doc: "buffer 3" },
    Builtin { mode: "buffer", key: "4", keys: ": neokak-buffer-nth 4<ret>", doc: "buffer 4" },
    Builtin { mode: "buffer", key: "5", keys: ": neokak-buffer-nth 5<ret>", doc: "buffer 5" },
    Builtin { mode: "buffer", key: "6", keys: ": neokak-buffer-nth 6<ret>", doc: "buffer 6" },
    Builtin { mode: "buffer", key: "7", keys: ": neokak-buffer-nth 7<ret>", doc: "buffer 7" },
    Builtin { mode: "buffer", key: "8", keys: ": neokak-buffer-nth 8<ret>", doc: "buffer 8" },
    Builtin { mode: "buffer", key: "9", keys: ": neokak-buffer-nth 9<ret>", doc: "buffer 9" },
    Builtin { mode: "window", key: "v", keys: ": neokak-window vsplit<ret>", doc: "split right" },
    Builtin { mode: "window", key: "s", keys: ": neokak-window split<ret>", doc: "split below" },
    Builtin { mode: "window", key: "h", keys: ": neokak-window h<ret>", doc: "focus left" },
    Builtin { mode: "window", key: "j", keys: ": neokak-window j<ret>", doc: "focus down" },
    Builtin { mode: "window", key: "k", keys: ": neokak-window k<ret>", doc: "focus up" },
    Builtin { mode: "window", key: "l", keys: ": neokak-window l<ret>", doc: "focus right" },
    Builtin { mode: "window", key: "w", keys: ": neokak-window next<ret>", doc: "next pane" },
    Builtin { mode: "window", key: "q", keys: ": neokak-window close<ret>", doc: "close pane" },
    Builtin { mode: "window", key: "o", keys: ": neokak-window only<ret>", doc: "close other panes" },
    Builtin { mode: "window", key: "<gt>", keys: ": neokak-window resize-right<ret>", doc: "wider" },
    Builtin { mode: "window", key: "<lt>", keys: ": neokak-window resize-left<ret>", doc: "narrower" },
    Builtin { mode: "window", key: "+", keys: ": neokak-window resize-up<ret>", doc: "taller" },
    Builtin { mode: "window", key: "<minus>", keys: ": neokak-window resize-down<ret>", doc: "shorter" },
    Builtin { mode: "window", key: "=", keys: ": neokak-window equalize<ret>", doc: "equal sizes" },
    Builtin { mode: "lsp", key: "s", keys: ": lsp-document-symbol<ret>", doc: "document symbols" },
    Builtin { mode: "lsp", key: "S", keys: ": lsp-workspace-symbol ", doc: "workspace symbols (type a query)" },
    Builtin { mode: "lsp", key: "r", keys: ": neokak-lsp References lsp-references<ret>", doc: "references" },
    Builtin {
        mode: "lsp",
        key: "R",
        keys: ": neokak-lsp context:References lsp-references<ret>",
        doc: "references with context",
    },
    Builtin { mode: "lsp", key: "d", keys: ": lsp-diagnostics<ret>", doc: "diagnostics" },
    Builtin {
        mode: "lsp",
        key: "i",
        keys: ": neokak-lsp Implementations lsp-implementation<ret>",
        doc: "implementations",
    },
    Builtin {
        mode: "lsp",
        key: "t",
        keys: ": neokak-lsp %{Type Definitions} lsp-type-definition<ret>",
        doc: "type definition",
    },
    Builtin { mode: "git", key: "g", keys: ": neokak-float %opt{neokak_git_tool}<ret>", doc: "git TUI (lazygit)" },
    Builtin { mode: "git", key: "s", keys: ": neokak-picker git-status<ret>", doc: "changed files" },
    Builtin { mode: "git", key: "n", keys: ": git next-hunk<ret>", doc: "next change" },
    Builtin { mode: "git", key: "p", keys: ": git prev-hunk<ret>", doc: "previous change" },
    Builtin { mode: "git", key: "b", keys: ": neokak-git-blame-toggle<ret>", doc: "toggle line blame" },
    Builtin { mode: "git", key: "B", keys: ": git blame<ret>", doc: "blame the whole file" },
    Builtin { mode: "git", key: "d", keys: ": git diff %val{buffile}<ret>", doc: "diff of this file" },
    Builtin { mode: "session", key: "s", keys: ": neokak-session-restore<ret>", doc: "restore this project" },
    Builtin { mode: "session", key: "p", keys: ": neokak-picker projects<ret>", doc: "switch project" },
    Builtin { mode: "session", key: "d", keys: ": neokak-session-forget<ret>", doc: "forget this project's session" },
    Builtin { mode: "surround", key: "a", keys: ": neokak-surround-add<ret>", doc: "add a pair around the selections" },
    Builtin { mode: "surround", key: "d", keys: ": neokak-surround-delete<ret>", doc: "delete the surrounding pair" },
    Builtin { mode: "surround", key: "r", keys: ": neokak-surround-replace<ret>", doc: "replace the surrounding pair" },
    Builtin { mode: "fold", key: "z", keys: ": neokak-fold-toggle<ret>", doc: "toggle fold" },
    Builtin { mode: "fold", key: "c", keys: ": neokak-fold<ret>", doc: "fold" },
    Builtin { mode: "fold", key: "o", keys: ": neokak-unfold<ret>", doc: "unfold" },
    Builtin { mode: "fold", key: "R", keys: ": neokak-unfold-all<ret>", doc: "unfold all" },
];

/// Splits a key sequence into Kakoune keys: `g<minus>` → `g`, `<minus>`.
pub fn key_tokens(spec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' && chars.clone().any(|c| c == '>') && chars.peek() != Some(&'>') {
            let mut tok = String::from('<');
            for c in chars.by_ref() {
                tok.push(c);
                if c == '>' {
                    break;
                }
            }
            out.push(tok);
        } else {
            out.push(c.to_string());
        }
    }
    out
}

/// A `[keys]` entry: the command and an optional description, or a problem.
type KeyEntry = Result<(String, Option<String>), String>;

/// The parsed config file.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub doc: DocumentMut,
}

/// Where the config file lives.
pub fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("neokak/config.toml"))
}

impl Config {
    /// Reads `file` (an empty config if it doesn't exist).
    pub fn load(file: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(file) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", file.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
            format!("config.toml: {}", e.to_string().lines().next().unwrap_or("parse error"))
        })?;
        Ok(Self { doc })
    }

    /// Writes the file, creating its directory.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension("toml.tmp");
        std::fs::write(&tmp, self.doc.to_string())?;
        std::fs::rename(tmp, file)
    }

    /// A setting's value in the file, if set.
    pub fn get(&self, s: &Setting) -> Option<&Value> {
        self.doc.get(s.section)?.get(s.key)?.as_value()
    }

    /// The value in effect: the file's, or the default.
    pub fn value(&self, s: &Setting) -> Value {
        self.get(s).filter(|v| s.check(v).is_ok()).cloned().unwrap_or_else(|| s.default_value())
    }

    pub fn set(&mut self, s: &Setting, v: Value) {
        let table = self.table(s.section);
        table[s.key] = Item::Value(v);
    }

    /// Removes a setting from the file, back to its default.
    pub fn reset(&mut self, s: &Setting) {
        if let Some(t) = self.doc.get_mut(s.section).and_then(Item::as_table_like_mut) {
            t.remove(s.key);
        }
    }

    fn table(&mut self, name: &str) -> &mut Table {
        if !self.doc.contains_table(name) {
            let mut table = Table::new();
            // In a file without tables, comments are the document's trailing
            // text, which would end up below the new table.
            if self.doc.iter().next().is_none() {
                let trailing = self.doc.trailing().as_str().unwrap_or_default().to_owned();
                if !trailing.trim().is_empty() {
                    table.decor_mut().set_prefix(trailing);
                    self.doc.set_trailing("");
                }
            }
            self.doc[name] = Item::Table(table);
        }
        self.doc[name].as_table_mut().expect("just made a table")
    }

    /// Problems with the file's settings and keys (unknown names, wrong
    /// types); those entries are ignored.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (section, item) in self.doc.iter() {
            if section == "keys" {
                continue;
            }
            let Some(table) = item.as_table_like() else {
                out.push(format!("{section}: expected a table"));
                continue;
            };
            for (key, item) in table.iter() {
                match SETTINGS.iter().find(|s| s.section == section && s.key == key) {
                    None => out.push(format!("{section}.{key}: unknown setting")),
                    Some(s) => match item.as_value() {
                        Some(v) => out.extend(s.check(v).err()),
                        None => out.push(format!("{section}.{key}: expected a value")),
                    },
                }
            }
        }
        let (_, key_problems) = self.bindings();
        out.extend(key_problems);
        out
    }

    /// The `[keys]` entries: key sequence, and the command (empty to remove
    /// the binding) with an optional description.
    fn key_entries(&self) -> Vec<(String, KeyEntry)> {
        let Some(table) = self.doc.get("keys").and_then(Item::as_table_like) else { return Vec::new() };
        table
            .iter()
            .map(|(spec, item)| {
                let entry = match item.as_value() {
                    Some(Value::String(s)) => Ok((s.value().clone(), None)),
                    Some(Value::InlineTable(t)) => match t.get("command").and_then(Value::as_str) {
                        Some(cmd) => {
                            Ok((cmd.to_owned(), t.get("description").and_then(Value::as_str).map(String::from)))
                        }
                        None => Err(format!("keys.{spec}: needs a command")),
                    },
                    _ => Err(format!("keys.{spec}: expected a command string")),
                };
                (spec.to_owned(), entry)
            })
            .collect()
    }

    /// The leader keymap in effect, and problems with `[keys]`.
    pub fn bindings(&self) -> (Vec<Binding>, Vec<String>) {
        let mut out: Vec<Binding> = BUILTIN_KEYS
            .iter()
            .map(|b| Binding { mode: b.mode.into(), key: b.key.into(), keys: b.keys.into(), doc: b.doc.into() })
            .collect();
        let mut problems = Vec::new();
        for (spec, entry) in self.key_entries() {
            let (command, doc) = match entry {
                Ok(e) => e,
                Err(e) => {
                    problems.push(e);
                    continue;
                }
            };
            let Some((mode, key)) = resolve(&out, &spec) else {
                problems.push(format!("keys.{spec}: use one key, or a group key and one key (e.g. \"gx\")"));
                continue;
            };
            let existing = out.iter().position(|b| b.mode == mode && b.key == key);
            if command.is_empty() {
                if let Some(i) = existing {
                    out.remove(i);
                }
                continue;
            }
            let keys = format!(": {}<ret>", command.replace('<', "<lt>"));
            let doc = doc.unwrap_or_else(|| command.clone());
            match existing {
                Some(i) => {
                    let b = &mut out[i];
                    b.keys = keys;
                    b.doc = doc;
                }
                None => out.push(Binding { mode, key, keys, doc }),
            }
        }
        (out, problems)
    }

    /// The command a binding runs, if it's a plain `: command<ret>`.
    pub fn command_of(b: &Binding) -> Option<String> {
        let cmd = b.keys.strip_prefix(": ")?.strip_suffix("<ret>")?;
        (!cmd.contains("<ret>")).then(|| cmd.replace("<lt>", "<"))
    }

    /// Sets (or with an empty command, removes) the binding for a leader
    /// key sequence in `[keys]`.
    pub fn set_key(&mut self, spec: &str, command: &str) {
        let table = self.table("keys");
        table[spec] = toml_edit::value(command);
    }

    /// Drops a key sequence from `[keys]`, back to the built-in binding.
    pub fn reset_key(&mut self, spec: &str) {
        if let Some(t) = self.doc.get_mut("keys").and_then(Item::as_table_like_mut) {
            t.remove(spec);
        }
    }

    pub fn has_key(&self, spec: &str) -> bool {
        self.doc.get("keys").and_then(|t| t.get(spec)).is_some()
    }

    /// The kakscript applying the file. `applied` holds the settings and
    /// bindings the previous run set; ones gone since go back to their
    /// defaults. Returns the script and what it applied.
    pub fn kakscript(&self, applied: &Applied) -> (String, Applied) {
        let mut out = String::new();
        let mut now = Applied::default();
        for s in SETTINGS {
            let value = match self.get(s) {
                Some(v) if s.check(v).is_ok() => {
                    now.settings.push(s.path());
                    v.clone()
                }
                // Back to the default once it's no longer in the file.
                _ if applied.settings.contains(&s.path()) => s.default_value(),
                _ => continue,
            };
            out.push_str(&format!("try {}\n", quote(&s.command(&value))));
        }
        let (bindings, _) = self.bindings();
        for (mode, key) in &applied.keys {
            out.push_str(&format!("try {}\n", quote(&format!("unmap global {mode} {}", key_word(key)))));
        }
        for b in bindings {
            let map =
                format!("map global {} {} {} -docstring {}", b.mode, key_word(&b.key), quote(&b.keys), quote(&b.doc));
            out.push_str(&format!("try {}\n", quote(&map)));
            now.keys.push((b.mode, b.key));
        }
        (out, now)
    }
}

/// A key as a single Kakoune word.
fn key_word(key: &str) -> String {
    if key.chars().any(|c| c.is_whitespace() || "'\"%{}".contains(c)) { quote(key) } else { key.to_owned() }
}

/// The user mode and key for a leader key sequence: `t` → (user, t),
/// `gx` → (git, x).
fn resolve(bindings: &[Binding], spec: &str) -> Option<(String, String)> {
    match key_tokens(spec).as_slice() {
        [key] => Some(("user".into(), key.clone())),
        [group, key] => {
            let b = bindings.iter().find(|b| b.mode == "user" && b.key == *group)?;
            let mode = b.keys.strip_prefix(": enter-user-mode ")?.strip_suffix("<ret>")?;
            Some((mode.to_owned(), key.clone()))
        }
        _ => None,
    }
}

/// The leader key sequence for a binding (`gx` for git's `x`).
pub fn spec_of(bindings: &[Binding], b: &Binding) -> String {
    if b.mode == "user" {
        return b.key.clone();
    }
    let group = bindings
        .iter()
        .find(|g| g.mode == "user" && g.keys == format!(": enter-user-mode {}<ret>", b.mode))
        .map_or_else(|| format!("{}:", b.mode), |g| g.key.clone());
    format!("{group}{}", b.key)
}

/// What a config run set, to undo what later disappears from the file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Applied {
    pub settings: Vec<String>,
    pub keys: Vec<(String, String)>,
}

/// A commented starter file listing every setting with its default.
pub fn template() -> String {
    let mut out = String::from(
        "# neokak settings. Uncomment a line to change it; neokak applies the file\n\
         # whenever it's saved. <space>o opens the settings screen.\n",
    );
    let mut section = "";
    for s in SETTINGS {
        if s.section != section {
            section = s.section;
            out.push_str(&format!("\n[{section}]\n"));
        }
        out.push_str(&format!("# {}\n", s.doc));
        for (i, line) in format!("{} = {}", s.key, s.default).lines().enumerate() {
            out.push_str(if i == 0 { "# " } else { "#" });
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(
        "\n# Leader keys: a key, or a group key and a key (\"gx\" is <space>g x), and the\n\
         # Kakoune command to run. An empty command removes a binding.\n\
         [keys]\n\
         # t = \"neokak-msg terminal\"\n\
         # gx = { command = \"git checkout %val{buffile}\", description = \"discard changes\" }\n\
         # J = \"\"\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_and_check() {
        for s in SETTINGS {
            assert!(s.check(&s.default_value()).is_ok(), "{}", s.path());
        }
    }

    #[test]
    fn the_template_is_valid_and_commented_out() {
        let c = Config::parse(&template()).unwrap();
        assert!(c.problems().is_empty(), "{:?}", c.problems());
        assert_eq!(c.kakscript(&Applied::default()).1.settings.len(), 0);
    }

    #[test]
    fn rc_defaults_match() {
        let rc = crate::RC;
        let providers = SETTINGS.iter().find(|s| s.key == "providers").unwrap().default_value();
        for p in providers.as_array().unwrap() {
            let p = p.as_str().unwrap();
            assert!(rc.contains(&format!("'{p}'")), "rc/neokak.kak lacks the default provider {p}");
        }
    }

    #[test]
    fn splits_keys() {
        assert_eq!(key_tokens("g<minus>"), ["g", "<minus>"]);
        assert_eq!(key_tokens("<space>"), ["<space>"]);
        assert_eq!(key_tokens("<"), ["<"]);
        assert_eq!(key_tokens("<>"), ["<", ">"]);
    }

    #[test]
    fn settings_become_options_and_revert_when_removed() {
        let c =
            Config::parse("[terminal]\nshell = \"fish\"\n[theme]\nname = \"nord\"\n[editor]\ntabstop = 4\n").unwrap();
        assert!(c.problems().is_empty());
        let (script, applied) = c.kakscript(&Applied::default());
        assert!(
            script.contains(&format!("try {}", quote("set-option global neokak_terminal_shell 'fish'"))),
            "{script}"
        );
        assert!(script.contains(&quote("set-option global neokak_theme 'nord'")));
        assert!(script.contains("set-option global tabstop 4"));
        assert!(!script.contains("neokak_git_signs"), "unset settings are left alone");

        let (script, _) = Config::parse("[editor]\ntabstop = 4\n").unwrap().kakscript(&applied);
        assert!(script.contains(&quote("set-option global neokak_terminal_shell ''")), "{script}");
        assert!(script.contains(&quote("set-option global neokak_theme 'gruvbox'")));
    }

    #[test]
    fn reports_problems() {
        let c =
            Config::parse("[theme]\nname = \"pink\"\ntransparent = \"yes\"\nnope = 1\n[keys]\nabc = \"x\"\n").unwrap();
        let p = c.problems();
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(p.iter().any(|e| e.starts_with("theme.name: expected one of")));
        assert!(Config::parse("[theme\n").is_err());
    }

    #[test]
    fn keys_override_add_and_remove_bindings() {
        let c = Config::parse(
            "[keys]\nt = \"echo <hi>\"\ngx = { command = \"git checkout\", description = \"discard\" }\nJ = \"\"\n",
        )
        .unwrap();
        let (bindings, problems) = c.bindings();
        assert!(problems.is_empty(), "{problems:?}");
        let t = bindings.iter().find(|b| b.mode == "user" && b.key == "t").unwrap();
        assert_eq!(t.keys, ": echo <lt>hi><ret>");
        assert_eq!(Config::command_of(t).as_deref(), Some("echo <hi>"));
        let gx = bindings.iter().find(|b| b.mode == "git" && b.key == "x").unwrap();
        assert_eq!(gx.doc, "discard");
        assert_eq!(spec_of(&bindings, gx), "gx");
        assert!(!bindings.iter().any(|b| b.mode == "user" && b.key == "J"));

        let (script, applied) = c.kakscript(&Applied::default());
        assert!(script.contains(&quote("map global git x ': git checkout<ret>' -docstring 'discard'")), "{script}");
        assert!(!script.contains("map global user J "));
        let (script, _) = Config::default().kakscript(&applied);
        assert!(script.contains("try 'unmap global git x'"));
        assert!(script.contains("map global user J "));
    }

    #[test]
    fn edits_keep_comments() {
        let mut c = Config::parse("# mine\n[theme]\n# pick one\nname = \"nord\"\n").unwrap();
        let name = SETTINGS.iter().find(|s| s.path() == "theme.name").unwrap();
        let shell = SETTINGS.iter().find(|s| s.path() == "terminal.shell").unwrap();
        c.set(name, "onedark".into());
        c.set(shell, "fish".into());
        c.set_key("t", "neokak-msg terminal");
        let text = c.doc.to_string();
        assert!(text.contains("# mine") && text.contains("# pick one"), "{text}");
        assert!(text.contains("name = \"onedark\""));
        let c = Config::parse(&text).unwrap();
        assert_eq!(c.value(shell).as_str(), Some("fish"));
        assert!(c.has_key("t"));
        let mut c = c;
        c.reset(name);
        assert_eq!(c.value(name).as_str(), Some("gruvbox"));
    }
}
