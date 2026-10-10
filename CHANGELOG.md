# Changelog

## Unreleased

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
