use std::sync::mpsc::Sender;

use crate::kak::rpc::KakEvent;
use crate::pane::PaneId;

/// Everything the main loop reacts to, funneled through one channel.
#[allow(clippy::large_enum_variant)] // Kak events dominate; boxing buys nothing.
pub enum AppEvent {
    Kak(PaneId, KakEvent),
    KakExited(PaneId),
    /// SIGHUP/SIGTERM: the terminal went away or someone asked us to stop.
    Quit,
    Term(crossterm::event::Event),
    /// Side-channel message from kakscript (`neokak msg ...`).
    Ipc(Vec<String>),
    /// Output from a floating terminal.
    PtyOutput(TermSlot, Vec<u8>),
    PtyExited(TermSlot),
    /// `git blame` result for a pane's cursor line.
    Blame {
        client: String,
        file: String,
        line: usize,
        text: Option<String>,
    },
    /// A project's environment provider finished.
    ProjectEnv {
        dir: std::path::PathBuf,
        provider: String,
        env: Result<Option<crate::projenv::Env>, String>,
    },
    /// Background work (fuzzy matcher, grep) has new results.
    Wake,
    /// Results of a live-grep run, tagged with the query generation.
    GrepResults {
        generation: u64,
        lines: Vec<String>,
        done: bool,
    },
}

/// Which floating terminal an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermSlot {
    /// The toggleable shell (`<space>t`), numbered so that output from a
    /// replaced shell is ignored.
    Shell(u64),
    /// A program run in a float until it exits (lazygit, `neokak-float`).
    Tool,
}

pub type Tx = Sender<AppEvent>;
