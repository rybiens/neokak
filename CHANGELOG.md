# Changelog

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
