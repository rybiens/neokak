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
- **Completion and hover popups** with rounded borders that suit transparent
  terminals
- **Which-key**: press the leader and a panel lists the available keys
- **Telescope-style pickers** with fuzzy matching and a preview pane: files,
  live grep (ripgrep), buffers, recent files, colorschemes, plus your own
- **File explorer** (neo-tree style) with git status, plus create, rename and
  delete
- **Floating terminal** (toggleterm style) that keeps your shell running when
  hidden
- **Split panes**: each pane is a separate Kakoune client on the same session,
  so buffers, registers and LSP are shared
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
  - `git` for the branch in the statusline, git status in the explorer, and
    the plugin manager
  - [kakoune-lsp](https://github.com/kakoune-lsp/kakoune-lsp) (`kak-lsp`) for
    LSP. If your kakrc doesn't load it, neokak loads it when it's installed.
  - A [Nerd Font](https://www.nerdfonts.com) for icons (otherwise run with
    `--ascii`)

## Install

```sh
git clone https://github.com/rybiens/neokak.git
cd neokak
cargo install --path .
```

This puts `neokak` in `~/.cargo/bin`. To build without installing, run
`cargo build --release` and use `./target/release/neokak`.

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

| Setting                                         | Default | Effect                    |
| ----------------------------------------------- | ------- | ------------------------- |
| `set-option global neokak_hide_hints_in_insert` | `true`  | Hide LSP inlay hints in insert mode (see [Known issues](#known-issues)) |

| Environment variable | Effect                                                    |
| -------------------- | --------------------------------------------------------- |
| `NEOKAK_ASCII=1`     | Same as `--ascii`                                         |
| `NEOKAK_DEBUG=1`     | Log frontend events to `~/.local/state/neokak/debug.log`  |
| `NEOKAK_NO_LSP=1`    | Don't load kak-lsp (if your kakrc loads it, it still runs) |

The colors are a fixed gruvbox-dark palette for now. Text inside the editor
uses your Kakoune colorscheme.

## Keys

The leader is `<space>` (Kakoune's user mode). Press it and wait to see every
mapping. All of Kakoune's normal keys work as usual.

| Keys                       | Action                               |
| -------------------------- | ------------------------------------ |
| `<space><space>`           | Find files                           |
| `<space>,`                 | Switch buffer                        |
| `<space>ff` / `<space>fg`  | Find files / live grep               |
| `<space>fb` / `<space>fr`  | Buffers / recent files               |
| `<space>fc` / `<space>fp`  | Colorschemes / list of all pickers   |
| `<space>bn` / `<space>bp`  | Next / previous buffer               |
| `<space>bd` / `<space>bD`  | Delete buffer / delete without saving |
| `<space>e`                 | File explorer: open, focus, or close if focused |
| `<space>t`, `<c-\>`        | Toggle the floating terminal         |
| `<space>\|` / `<space>-`   | Split right / split below            |
| `<space>wh/j/k/l`          | Move between panes (and into the explorer) |
| `<space>ww`                | Next pane                            |
| `<space>wq` / `<space>wo`  | Close pane / close all other panes   |

**In a picker:** type to filter. `<c-n>`/`<c-p>` (or arrows) move, `<ret>`
opens, `<esc>` closes, `<c-u>` clears the query and `<c-w>` deletes a word.

**In the explorer:** `j`/`k` move, `l` or `<ret>` opens a file or expands a
folder, `h` collapses, `a` creates a file (end the name with `/` for a
folder), `r` renames, `d` deletes, `H` shows dotfiles, `R` refreshes, `q`
closes and `<esc>` returns to the editor.

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
- **No theme configuration yet.** The interface uses a fixed gruvbox palette.

## Roadmap

- Configurable themes, or deriving the interface colors from the Kakoune
  colorscheme
- Resizable splits and explorer
- LSP pickers (symbols, references, diagnostics)
- Git integration beyond status (hunks, blame)

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
