# neokak

[![CI](https://github.com/rybiens/neokak/actions/workflows/ci.yml/badge.svg)](https://github.com/rybiens/neokak/actions/workflows/ci.yml)

A Neovim-style terminal IDE built on [Kakoune](https://kakoune.org).

Kakoune does all the editing. neokak runs it through Kakoune's JSON UI
(`kak -ui json`) and draws everything around it, so it looks and feels like a
fully configured Neovim (LazyVim, AstroNvim) while keeping Kakoune's editing
model, your kakrc and your Kakoune plugins.

> [!WARNING]
> **This project is vibe coded.** It was written almost entirely by an AI
> coding assistant (Claude), with a human steering, testing and reviewing the
> behavior, not every line of code. It works for its author, but it hasn't had
> a careful human code review or broad real-world use. Expect rough edges, and
> read the code before relying on it for anything important. Bug reports and
> fixes are welcome.

## Features

- **Bufferline** of open buffers (click to switch)
- **Lualine-style statusline**: mode, git branch, file, modified marker, LSP
  error and warning counts, filetype, position
- **Floating command line**, with completion and command help in floats
- **Completion and hover popups** with rounded borders. Hover docs float just
  above the text they describe.
- **Themes**: six built-in interface themes plus one that follows your
  Kakoune colorscheme, with live switching, per-color overrides, and
  optional transparent popups
- **Which-key**: press the leader and a panel lists the available keys
- **Telescope-style pickers** with fuzzy matching and a preview pane: files,
  live grep (ripgrep), buffers, recent files, colorschemes, themes, plus your
  own
- **LSP pickers**: document symbols, references, diagnostics, implementations
  and other kak-lsp result lists open in a picker with a preview
- **Tree-sitter** (with [kak-tree-sitter](https://git.sr.ht/~hadronized/kak-tree-sitter)):
  syntax highlighting in your colorscheme's colors and function/type text
  objects
- **Folding**: fold the function (tree-sitter) or indented block at the
  cursor; folded lines open while the cursor is inside them
- **Function and symbol navigation**: jump to the next or previous function
  or symbol
- **References with context**: every reference with the code around it, in
  one navigable buffer
- **Git**: live change markers in the gutter, hunk navigation, inline blame
  for the cursor line, a changed-files picker, and lazygit (or any tool) in a
  floating terminal
- **File explorer** (neo-tree style) with git status, plus create, rename and
  delete
- **Floating terminal** (toggleterm style) that keeps your shell running when
  hidden
- **Split panes**: each pane is a separate Kakoune client on the same session,
  so buffers, registers and LSP are shared. Resize panes and the explorer with
  Ctrl+arrows or by dragging their edges.
- **Project sessions**: the files open in each project (and the cursor in
  each) are saved as you work; restore them, or switch projects, from the
  dashboard or `<space>q`
- **Project environments**: devenv, direnv, or any tool you configure is
  activated for the project, so the terminal and language servers get the
  project's tools
- **Start dashboard** when opened without files
- **Notification toasts**
- **Plugin manager** for Kakoune plugins, plus a small kakscript API for adding
  pickers and notifications

## Status

Early and experimental (v0.1). It has only been tested on Linux (Arch) with
Kakoune 2026.05.21. Other Unix systems may work but haven't been tried.
Windows is not supported.

## Requirements

- [Kakoune](https://kakoune.org), recent enough for the JSON UI protocol it
  uses (tested with 2026.05.21)
- Rust 1.88 or newer, to build
- Optional:
  - [ripgrep](https://github.com/BurntSushi/ripgrep) (`rg`) for live grep
  - `git` for the branch in the statusline, change markers, blame, git status
    in the explorer, and the plugin manager
  - [kak-tree-sitter](https://git.sr.ht/~hadronized/kak-tree-sitter) for
    tree-sitter highlighting, text objects and function folds (see
    [Tree-sitter](#tree-sitter))
  - [lazygit](https://github.com/jesseduffield/lazygit) for `<space>gg` (or
    set `neokak_git_tool` to another program)
  - [kakoune-lsp](https://github.com/kakoune-lsp/kakoune-lsp) (`kak-lsp`) for
    LSP. If your kakrc doesn't load it, neokak loads it when it's installed.
  - A [Nerd Font](https://www.nerdfonts.com) for icons (otherwise run with
    `--ascii`)

## Install

### Prebuilt binary (x86_64 Linux)

Download `neokak-<version>-x86_64-linux.tar.gz` from the
[releases page](https://github.com/rybiens/neokak/releases). It's statically
linked, so it runs on any distribution. Put the binary somewhere on your PATH:

```sh
tar xzf neokak-v0.1.0-x86_64-linux.tar.gz
install -m 755 neokak-v0.1.0-x86_64-linux/neokak ~/.local/bin/
```

Each archive has a `.sha256` file next to it for checking the download
(`sha256sum -c neokak-v0.1.0-x86_64-linux.tar.gz.sha256`). Kakoune itself
needs to be installed separately.

### From source

```sh
git clone https://github.com/rybiens/neokak.git
cd neokak
cargo install --locked --path .
```

This installs to `~/.cargo/bin`. If that isn't on your PATH (common with
distro-packaged rustup), either add it or install somewhere that is:

```sh
cargo install --locked --path . --root ~/.local   # installs ~/.local/bin/neokak
```

### With Nix

The flake's package includes Kakoune, git, ripgrep and perl as fallbacks, so
it works without installing them separately (versions already on your PATH
take precedence).

```sh
nix run github:rybiens/neokak             # try it
nix profile install github:rybiens/neokak  # install it
```

In a flake-based NixOS or Home Manager configuration, add the input and its
package:

```nix
inputs.neokak = {
  url = "github:rybiens/neokak";
  inputs.nixpkgs.follows = "nixpkgs";
};

# then, in a module:
home.packages = [ inputs.neokak.packages.${pkgs.stdenv.hostPlatform.system}.default ];
```

`nix develop` gives a shell with the Rust toolchain, Kakoune and ripgrep for
working on neokak.

## Usage

```sh
neokak [--clean] [--ascii] [files...]
```

| Option          | Effect                                                  |
| --------------- | ------------------------------------------------------- |
| `--clean`       | Skip `~/.config/kak` (load only neokak's defaults)      |
| `--ascii`       | Plain text instead of Nerd Font icons                   |
| `-h`, `--help`  | Show help                                               |
| `-V`, `--version` | Show the version                                      |

Any other arguments go straight to `kak`, so `neokak +42 src/main.rs` opens
that file at line 42.

## Configuration

neokak loads your configuration in this order:

1. Your normal Kakoune config (`~/.config/kak/kakrc` and its autoload), unless
   you pass `--clean`.
2. neokak's defaults: leader keys, statusline hooks, LSP setup.
3. `~/.config/neokak/init.kak`, if it exists. Put neokak-specific settings and
   plugin declarations here. The dashboard's **Config** button opens it.

Because neokak's defaults load after your kakrc, its `<space>f`, `<space>b`,
`<space>e`, `<space>t`, `<space>w`, `<space>|` and `<space>-` mappings replace
any of yours on those keys. Remap them in `init.kak` if you want them back.

| Option                        | Default   | Effect                                         |
| ----------------------------- | --------- | ---------------------------------------------- |
| `neokak_theme`                | `gruvbox` | Interface theme (see [Themes](#themes))        |
| `neokak_transparent`          | `true`    | Draw popups on the terminal background         |
| `neokak_theme_colors`         | (empty)   | Per-color theme overrides                      |
| `neokak_git_signs`            | `true`    | Git change markers in the gutter               |
| `neokak_git_blame_line`       | `false`   | Show who last changed the cursor line          |
| `neokak_git_tool`             | `lazygit` | Program `<space>gg` runs                       |
| `neokak_session`              | `true`    | Save the open files of each project            |
| `neokak_session_autorestore`  | `false`   | Restore the session when started without files |
| `neokak_env_providers`        | direnv, devenv | How to load project environments (see [Project environments](#project-environments)) |
| `neokak_ts_faces`             | `true`    | Link tree-sitter faces to your colorscheme's   |
| `neokak_lsp_pickers`          | `true`    | Show kak-lsp result lists in pickers instead of buffers |
| `neokak_hide_hints_in_insert` | `true`    | Hide LSP inlay hints in insert mode (see [Known issues](#known-issues)) |

Set them with `set-option global <option> <value>` in `init.kak`.

| Environment variable | Effect                                                    |
| -------------------- | --------------------------------------------------------- |
| `NEOKAK_ASCII=1`     | Same as `--ascii`                                         |
| `NEOKAK_DEBUG=1`     | Log frontend events to `~/.local/state/neokak/debug.log`  |
| `NEOKAK_NO_LSP=1`    | Don't load kak-lsp (if your kakrc loads it, it still runs) |

### Tree-sitter

When [kak-tree-sitter](https://git.sr.ht/~hadronized/kak-tree-sitter) is on
your PATH, neokak starts it (unless your kakrc already does) and gets:

- **Highlighting** from tree-sitter grammars. kak-tree-sitter's `ts_*` faces
  are linked to Kakoune's standard faces (`function`, `keyword`, `string`,
  ...), so your usual colorscheme colors it. Using a colorscheme with its own
  `ts_*` faces? Set `neokak_ts_faces false`.
- **Text objects**: `<a-a>f` / `<a-i>f` select the function around the cursor
  (or its body), `<a-a>t` / `<a-i>t` the type (struct, class, impl).
- **Function folds**: `<space>zc` folds the whole function around the cursor.

Grammars are installed per language with `ktsctl sync rust nix ...`. With
home-manager:

```nix
home.packages = [ pkgs.kak-tree-sitter ];
home.activation.kakTreeSitterGrammars = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
  PATH="${lib.makeBinPath [ pkgs.git ]}:$PATH" \
    run ${pkgs.kak-tree-sitter}/bin/ktsctl sync rust nix toml markdown
'';
```

### Folding

Kakoune has no folding of its own, so neokak draws folds itself: a folded
block collapses into its first line followed by `··· N lines`. `<space>zc`
folds the function around the cursor when tree-sitter knows the language,
and otherwise the block indented deeper than the cursor line (or the block
the cursor is in). `<space>zz` toggles, `<space>zo` opens the fold at the
cursor and `<space>zR` opens them all.

Folded lines aren't skipped by cursor movement the way they are in Vim:
moving into a fold (or jumping into one from a search or a picker) shows its
lines, and they fold again once the cursor leaves.

### References with context

`<space>sR` lists every reference to the symbol under the cursor together
with three lines of code around each, grouped by file, in a `*references*`
buffer (in ripgrep's `-C` format). `<ret>` on a reference line jumps to it.

### Sessions

neokak treats the directory you start it in as the project. While you work,
it saves which files are open there, the cursor position in each, and which
one was showing, to `~/.local/state/neokak/sessions/`. Saving happens when
you switch files, save, and quit, so a session survives a closed window too.

- Start neokak in a project without files and press `s` on the dashboard
  (or `<space>qs` any time) to reopen that project's files.
- `<space>qp` (or `p` on the dashboard) lists every project with a saved
  session; picking one switches neokak and Kakoune to that directory and
  restores its files.
- `<space>qd` deletes the current project's session and stops saving it
  until neokak restarts.
- Set `neokak_session_autorestore true` to restore automatically whenever
  neokak starts without files.

Opening neokak on specific files (`neokak foo.rs`) also updates the project's
session, to just those files once you quit. Files that no longer exist are
skipped when restoring, and temporary copies from outside the project (such
as sudoedit's) are never saved.

Switching projects closes the files from outside the new project (scratch
buffers stay). If any of them have unsaved changes, neokak asks first: `y`
discards the changes, `w` saves them all, any other key stays in the current
project. It also starts the floating terminal over in the new directory,
unless something is running in it.

### Project environments

Tools like direnv and devenv activate a project's environment from a shell
hook when you `cd` into it. neokak does the same when you start it in a
project or switch to one: it loads the environment, restarts kak-lsp (and
so the language servers) with it, and starts the floating terminal and
floats like lazygit with it. neokak skips this when it was started from a
shell that already has the project's direnv or devenv environment active.

Environments come from *providers*, listed in `neokak_env_providers` as
`<marker>:<command>`. The first provider whose marker file is in the project
directory or a parent wins. Its command runs in the project directory and
prints the environment NUL-separated, the way `env -0` does; printing
nothing means it doesn't apply, and failing shows its last error line. The
defaults:

```kak
set-option global neokak_env_providers \
    '.envrc:if command -v direnv >/dev/null; then direnv exec . env -0; fi' \
    'devenv.nix:if command -v devenv >/dev/null; then devenv hook-should-activate >/dev/null && devenv shell -- env -0; fi'
```

devenv only activates projects you've trusted with `devenv allow`, as its
shell hook does, and direnv only runs `.envrc` files you've `direnv allow`ed.
Add other tools the same way:

```kak
set-option -add global neokak_env_providers \
    'mise.toml:mise exec -- env -0' \
    'flake.nix:nix develop --command env -0'
```

Clear the option to turn this off. Kakoune's own process environment can't
change once it runs, so commands it runs itself (`:make`, formatters) keep
neokak's starting environment. In your own `%sh{}` blocks, source
`$kak_opt_neokak_env` (a script that switches to the project environment)
to get the project's tools.

### Themes

The theme colors everything neokak draws: the bars, popups, pickers,
explorer and terminal frame. Text inside the editor keeps your Kakoune
colorscheme, so pick a theme that matches it.

Built-in themes: `gruvbox` (default), `gruvbox-light`, `tokyonight`,
`catppuccin`, `nord` and `onedark`. There's also `kakoune`, which takes its
colors from your Kakoune colorscheme and follows it when it changes: function
names become the accent color, keywords, strings and types the other accents,
comments the dim text, and the menu and status line faces the popup and bar
backgrounds. Faces your colorscheme leaves unset keep the gruvbox color.

```kak
# In ~/.config/neokak/init.kak
set-option global neokak_theme tokyonight

# Solid popup backgrounds instead of the terminal's (e.g. for opaque terminals).
set-option global neokak_transparent false

# Adjust single colors on top of the theme.
set-option global neokak_theme_colors blue=#7aa2f7 bg_sel=rgb:3b4261
```

Changes apply immediately, so you can also try themes from the command line
with `:neokak-theme <name>` (it tab-completes), or from the picker with
`<space>ft`. Colors you can override: `bg_float`, `bg_bar`, `bg_alt`,
`bg_sel`, `fg`, `fg_dim`, `border`, `red`, `green`, `yellow`, `blue`,
`purple`, `aqua` and `orange`. Colors are written `#rrggbb`, `rgb:rrggbb`, or
a terminal color name like `blue` or `bright-black`. An unknown theme or a
bad color shows an error notification.

## Keys

The leader is `<space>` (Kakoune's user mode). Press it and wait to see every
mapping. All of Kakoune's normal keys work as usual.

| Keys                       | Action                               |
| -------------------------- | ------------------------------------ |
| `<space><space>`           | Find files                           |
| `<space>,`                 | Switch buffer                        |
| `<space>ff` / `<space>fg`  | Find files / live grep               |
| `<space>fb` / `<space>fr`  | Buffers / recent files               |
| `<space>fc` / `<space>ft`  | Kakoune colorschemes / neokak themes |
| `<space>fp`                | List of all pickers                  |
| `<space>j` / `<space>k`    | Next / previous function             |
| `<space>J` / `<space>K`    | Next / previous symbol               |
| `<space>zz` / `<space>zc`  | Toggle fold / fold                   |
| `<space>zo` / `<space>zR`  | Unfold / unfold all                  |
| `<a-a>f` / `<a-a>t`        | Select function / type (tree-sitter) |
| `<space>ss` / `<space>sS`  | Document symbols / workspace symbols |
| `<space>sr` / `<space>sR`  | References / references with context |
| `<space>sd`                | Diagnostics                          |
| `<space>si` / `<space>st`  | Implementations / type definitions   |
| `<space>gg`                | lazygit in a floating terminal       |
| `<space>gs`                | Changed files                        |
| `<space>gn` / `<space>gp`  | Next / previous change               |
| `<space>gb` / `<space>gB`  | Toggle line blame / blame whole file |
| `<space>gd`                | Diff of the current file             |
| `<space>qs`                | Restore this project's session       |
| `<space>qp`                | Switch project                       |
| `<space>qd`                | Forget this project's session        |
| `<space>bn` / `<space>bp`  | Next / previous buffer               |
| `<space>bd` / `<space>bD`  | Delete buffer / delete without saving |
| `<space>e`                 | File explorer: open, focus, or close if focused |
| `<space>t`, `<c-\>`        | Toggle the floating terminal         |
| `<space>\|` / `<space>-`   | Split right / split below            |
| `<space>wh/j/k/l`          | Move between panes (and into the explorer) |
| `<space>ww`                | Next pane                            |
| `<space>wq` / `<space>wo`  | Close pane / close all other panes   |
| `<c-left>` / `<c-right>`   | Narrower / wider (pane or explorer)  |
| `<c-down>` / `<c-up>`      | Shorter / taller pane                |
| `<space>w=`                | Make all panes equal sizes           |

**In a picker:** type to filter. `<c-n>`/`<c-p>` (or arrows) move, `<ret>`
opens, `<esc>` closes, `<c-u>` clears the query and `<c-w>` deletes a word.

**In the explorer:** `j`/`k` move, `l` or `<ret>` opens a file or expands a
folder, `h` collapses, `a` creates a file (end the name with `/` for a
folder), `r` renames, `d` deletes, `H` shows dotfiles, `R` refreshes, `<` and `>` change its width, `q` or
`<space>e` hides it, and `<esc>` returns to the editor while leaving it open.
Leader mappings (`<space>…`) work from the explorer too.

**Which part has focus:** the statusline's mode pill reads `EXPLORER` or
`TERMINAL` when those have focus, the explorer's header lights up when it
does, and with split panes each pane has a title bar with its buffer name,
highlighted for the focused one.

**With the mouse:** drag a separator between panes, or the explorer's right
edge, to resize.

**In the terminal:** everything goes to the shell except `<c-\>`, which hides
it. The mouse wheel scrolls back through output.

## Plugins

Declare plugins in `~/.config/neokak/init.kak`. Give each a GitHub `user/repo`
or a full git URL, plus an optional config block:

```kak
neokak-plug alexherbo2/auto-pairs.kak %{ enable-auto-pairs }
neokak-plug andreyorst/smarttab.kak %{ require-module smarttab }
neokak-plug https://example.com/some/plugin.git
```

Then run `:neokak-plug-install`. Plugins are cloned into
`~/.local/share/neokak/plugins` and load right away. On later starts, every
`.kak` file in each plugin is sourced and then its config block runs.

| Command                | Effect                                            |
| ---------------------- | ------------------------------------------------- |
| `:neokak-plug-install` | Clone declared plugins that aren't installed yet  |
| `:neokak-plug-update`  | `git pull` every installed plugin (restart to reload) |
| `:neokak-plug-clean`   | Delete installed plugins you no longer declare    |

Plugins you already load from your kakrc keep working. You don't need to move
them.

## Kakscript API

These commands are available in neokak sessions, for your config or for
plugins that want to integrate with neokak.

```kak
# Register a picker. <source> is a shell command whose output lines are the
# items; the chosen item is passed as the last argument to <kak-command>.
neokak-picker-define <name> <title> <source> <kak-command>

# Open a picker: files, grep, buffers, recent, pickers, or one you defined.
neokak-picker <name>

# Show a notification toast. <level> is info (default), warn or error.
neokak-notify [<level>] <message>

# Switch the interface theme.
neokak-theme <name>

# Run a program in a floating terminal until it exits.
neokak-float <program> [<args>...]

# Folds.
neokak-fold
neokak-unfold
neokak-fold-toggle
neokak-unfold-all

# Sessions.
neokak-session-restore
neokak-session-load <dir>
neokak-session-forget

# Explorer and panes, as used by the default mappings.
neokak-explorer
neokak-window vsplit|split|close|only|next|h|j|k|l
```

For example, this is how the built-in colorscheme picker is defined:

```kak
neokak-picker-define colorschemes 'Colorschemes' \
    "ls '%val{runtime}/colors' | sed -n 's/\.kak$//p'" colorscheme
```

## How it works

```
neokak ──JSON-RPC over stdin/stdout──▶ kak -s neokak-<pid> -ui json   (session + "main" client)
neokak ──JSON-RPC over stdin/stdout──▶ kak -c neokak-<pid> -ui json   (one per extra split)
   ▲                                        │  hooks in rc/neokak.kak
   └─────── unix socket ◀── neokak msg ─────┘  (buffer list, current file, picker requests)
neokak ──kak -p──▶ session                     (open a file, switch buffer, ...)
```

- Kakoune sends each client's frame (buffer contents, status line, menus, info
  boxes) as JSON. neokak draws them with [ratatui](https://ratatui.rs) and
  sends keys, mouse events and resizes back.
- Things Kakoune doesn't report through the UI protocol (open buffers, the
  current file, requests to open a picker) come from kakscript hooks in
  [`rc/neokak.kak`](rc/neokak.kak). They call `neokak msg ...`, which writes
  to a Unix socket the frontend listens on.
- The statusline reads its fields from Kakoune's modeline. neokak sets
  `modelinefmt` to a record of fields separated by U+241E, so the mode,
  position and diagnostic counts always match the frame on screen.
- Pickers use [nucleo](https://github.com/helix-editor/nucleo) for fuzzy
  matching. The terminal uses [portable-pty](https://crates.io/crates/portable-pty)
  and [vt100](https://crates.io/crates/vt100).

The code is in `src/`:

| Path                 | Contents                                         |
| -------------------- | ------------------------------------------------ |
| `src/kak/`           | JSON-RPC protocol, client processes, `kak -p`     |
| `src/app.rs`         | State and event loop                             |
| `src/pane.rs`        | Panes and the split layout                       |
| `src/ui/`            | Every visible component                          |
| `src/ipc.rs`         | The `neokak msg` side channel                    |
| `rc/neokak.kak`      | Kakscript glue, default keys, plugin manager     |
| `tests/`             | Integration and end-to-end tests                 |

## Testing

```sh
cargo test
```

The suite has three layers:

- **Unit tests** (in `src/`): protocol parsing, key translation, layout,
  pickers, the file tree, git status, and rendering of each component into
  an off-screen buffer.
- **Protocol tests** (`tests/kak_protocol.rs`): neokak's client code against
  a real `kak -ui json`.
- **End-to-end tests** (`tests/e2e.rs`): the real `neokak` binary in a
  pseudo-terminal, driven by keystrokes, with the screen checked as text.
  They cover opening and saving files, which-key, the pickers, live grep, the
  explorer, splits, the terminal, and clean shutdown.

The kak-backed tests need `kak` on your PATH and are skipped without it (live
grep also needs `rg`). They run against temporary HOME and XDG directories,
so they never touch your config or running sessions.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs on every push
to `master` and on pull requests. Its **build** stage checks formatting, runs
clippy and builds on stable and on the minimum supported Rust (1.88). Its
**test** stage builds Kakoune from source and runs the whole suite.

## Troubleshooting

- **Something looks wrong or a key does nothing.** Check Kakoune's `*debug*`
  buffer (`:buffer *debug*`). Errors from your kakrc and plugins land there.
- **neokak exits right away or kak won't start.** Kakoune's stderr goes to
  `~/.local/state/neokak/kak.log`.
- **You need to see what the frontend receives.** Run with `NEOKAK_DEBUG=1`
  and read `~/.local/state/neokak/debug.log`.
- **Boxes or squares instead of icons.** Use a Nerd Font, or run with
  `--ascii`.
- **Esc followed quickly by a key acts like Alt plus that key.** This is the
  usual terminal escape ambiguity. Terminals with the kitty keyboard protocol
  (kitty, WezTerm, foot, Ghostty) don't have it. Inside tmux, a low
  `escape-time` helps.

## Known issues

- **Inlay hints after edits outside insert mode.** Kakoune doesn't move empty
  ranges (which LSP inlay hints are) when lines are inserted above them, so a
  hint can show on the wrong line until kak-lsp refreshes it about 200ms later.
  neokak hides hints in insert mode to cover the common case. Edits made from
  normal mode (`p`, `d`, `J`) can still flash a misplaced hint.
- **Nerd Font glyphs in the terminal.** Some icons in shell prompts may be
  measured at a different width than your terminal draws them, which can
  shift the terminal window's right border on those lines.
- **Mouse support is lightly tested.**

## Roadmap

- Live workspace-symbol search (results update as you type)
- Staging and resetting individual git hunks
- Restoring split layouts with sessions

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
