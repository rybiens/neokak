//! neokak: a Neovim-style IDE frontend for Kakoune.
//!
//! The binary in `main.rs` wires these modules to a real terminal; they are
//! a library so the test suite can drive them directly.

pub mod app;
pub mod debug;
pub mod event;
pub mod git;
pub mod ipc;
pub mod kak;
pub mod keys;
pub mod lsp;
pub mod pane;
pub mod recent;
pub mod ui;

use std::path::PathBuf;

/// Kakscript glue sourced into every session.
pub const RC: &str = include_str!("../rc/neokak.kak");

/// Where Kakoune's stderr is logged (`$XDG_STATE_HOME/neokak/kak.log`).
pub fn log_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    let dir = state.join("neokak");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("kak.log")
}
