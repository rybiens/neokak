//! Side channel from kakscript to the frontend.
//!
//! Hooks in `rc/neokak.kak` run `neokak msg <args...>`, which connects to the
//! frontend's Unix socket and writes its arguments NUL-separated.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread;

use anyhow::{Context, Result};

use crate::event::{AppEvent, Tx};

pub fn listen(path: &Path, tx: Tx) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = String::new();
            if stream.read_to_string(&mut buf).is_err() {
                continue;
            }
            let args = buf.split('\0').map(str::to_owned).collect();
            if tx.send(AppEvent::Ipc(args)).is_err() {
                return;
            }
        }
    });
    Ok(())
}

/// Client side, used by the `neokak msg` subcommand.
pub fn send(path: &Path, args: &[String]) -> Result<()> {
    let mut stream = UnixStream::connect(path).with_context(|| format!("connecting {}", path.display()))?;
    stream.write_all(args.join("\0").as_bytes())?;
    Ok(())
}
