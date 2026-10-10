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
    /// Output from the floating terminal's shell.
    PtyOutput(Vec<u8>),
    PtyExited,
    /// Background work (fuzzy matcher, grep) has new results.
    Wake,
    /// Results of a live-grep run, tagged with the query generation.
    GrepResults {
        generation: u64,
        lines: Vec<String>,
        done: bool,
    },
}

pub type Tx = Sender<AppEvent>;
