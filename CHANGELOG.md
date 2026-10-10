# Changelog

## Unreleased

- Project environments: starting neokak in a project or switching to one
  loads the project's environment (direnv and devenv out of the box, any
  tool through `neokak_env_providers`) and restarts kak-lsp, the terminal
  and floats with it.
- Fixed: after switching projects, the floating terminal stayed in the old
  project's directory.
- Switching projects closes the previous project's files, asking first when
  they have unsaved changes. Before, they stayed open and were saved into the
  new project's session.
- Tree-sitter: when kak-tree-sitter is installed, neokak starts it, links its
  faces to the colorscheme's standard faces, and maps `<a-a>f`/`<a-a>t` to
  function and type text objects.
- Folding (`<space>z`): fold the function around the cursor (tree-sitter) or
  the indented block; folds open while the cursor is inside them.
- `<space>j`/`<space>k` jump to the next/previous function, `<space>J`/
  `<space>K` to the next/previous symbol (kak-lsp, or kak-tree-sitter).
- `<space>sR` shows every reference with the code around it in a
  `*references*` buffer.
- Project sessions: the files open in each project directory and the cursor
  in each are saved as you work. Restore them from the dashboard (`s`) or
  with `<space>qs`, switch projects with `<space>qp` (or `p` on the
  dashboard), and forget a session with `<space>qd`. Optional automatic
  restore with `neokak_session_autorestore`.
- Fixed: quitting neokak (or closing its terminal) killed Kakoune while its
  shutdown hooks were running, which left kak-lsp and its language servers
  running after neokak exited. With rust-analyzer that leaked hundreds of MB
  per session. neokak now restores the terminal and lets Kakoune finish.
  This includes closing the terminal window (e.g. with a window manager
  keybind): Kakoune now runs in its own process group, so the terminal's
  hangup signal reaches neokak, which shuts Kakoune down cleanly.
- Clearer focus: the statusline shows `EXPLORER` or `TERMINAL` when those
  have focus, the explorer header and the bufferline's active marker
  highlight only while focused, and split panes get a title bar each, with
  the focused one highlighted.
- Fixed: leader mappings, including `<space>e` to hide the explorer, didn't
  work while the explorer had focus.
- A `kakoune` theme that takes the interface colors from the Kakoune
  colorscheme and updates when it changes.
- Git integration: change markers in the gutter that update as you type
  (using Kakoune's git tool), `<space>gn`/`<space>gp` to jump between
  changes, inline blame for the cursor line (`<space>gb`), a changed-files
  picker (`<space>gs`), whole-file blame and diff, and lazygit in a floating
  terminal (`<space>gg`, configurable with `neokak_git_tool`).
- `neokak-float <program>` runs any program in a floating terminal until it
  exits.
- LSP pickers: kak-lsp's result lists (document and workspace symbols,
  references, diagnostics, implementations, call hierarchy) open in a
  picker with a preview instead of a buffer. New `<space>s` mappings:
  `ss` symbols, `sS` workspace symbols, `sr` references, `sd` diagnostics,
  `si` implementations, `st` type definitions. Set
  `neokak_lsp_pickers false` for kak-lsp's buffers.
- Resizable panes and explorer: Ctrl+arrows (or `<space>w` then `<`, `>`,
  `+`, `-`) resize the focused pane or the explorer, `<space>w=` equalizes,
  and separators and the explorer's edge can be dragged with the mouse.
  `<` and `>` also resize the explorer while it has focus.
- Interface themes: `gruvbox`, `gruvbox-light`, `tokyonight`, `catppuccin`,
  `nord` and `onedark`, set with the `neokak_theme` option or switched live
  with `:neokak-theme` and the themes picker (`<space>ft`). Popup
  transparency is now the `neokak_transparent` option, and single colors can
  be overridden with `neokak_theme_colors`.
- Fixed: LSP hover text (and other untitled info boxes) showed up in the
  which-key panel at the bottom, with lines split at `": "`. It now floats
  above the cursor, or below it when there's no room above. Anchored info
  boxes also prefer the space above their anchor.

## v0.1.0

First release. neokak runs Kakoune through its JSON UI and draws a
Neovim-style interface around it.

- Bufferline, lualine-style statusline and a floating command line
- Completion, hover and which-key popups
- Telescope-style pickers: files, live grep, buffers, recent files,
  colorschemes, and custom pickers from kakscript
- Neo-tree style file explorer with git status and file operations
- Floating terminal
- Split panes, each a separate Kakoune client on the shared session
- Start dashboard and notification toasts
- Plugin manager for Kakoune plugins (`neokak-plug`)
- LSP inlay hints hidden in insert mode, to avoid a Kakoune bug that
  briefly draws them on the wrong line

The prebuilt binary is for x86_64 Linux and is statically linked, so it runs
on any distribution. It needs Kakoune installed separately.
