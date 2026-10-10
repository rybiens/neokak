//! The json-ui Kakoune client process that renders into neokak.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::rpc;
use super::session;
use crate::event::{AppEvent, Tx};
use crate::pane::PaneId;

pub struct KakClient {
    child: Child,
    /// Set for the session's server process once its client has left: the
    /// server keeps running for the other clients, so it must not be killed.
    detached: bool,
    stdin: ChildStdin,
    pub session: String,
}

impl KakClient {
    /// Starts `kak -ui json` as both session server and the `main` client.
    pub fn spawn(cmd: Command, session: String, stderr_log: &Path, pane: PaneId, tx: Tx) -> Result<Self> {
        let mut cmd = cmd;
        let log = std::fs::OpenOptions::new().create(true).append(true).open(stderr_log)?;
        // Keep Kakoune out of the terminal's process group: when the window
        // is closed, the terminal sends SIGHUP to that group, and Kakoune
        // would die without running its KakEnd hooks (orphaning kak-lsp).
        // neokak gets the SIGHUP instead and ends the session properly.
        cmd.process_group(0);
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .context("failed to start kak (is it installed and on PATH?)")?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                // Unparseable lines are skipped; stderr belongs to the TUI.
                if let Ok(ev) = rpc::parse(&line)
                    && tx.send(AppEvent::Kak(pane, ev)).is_err()
                {
                    return;
                }
            }
            let _ = tx.send(AppEvent::KakExited(pane));
        });

        Ok(Self { child, stdin, detached: false, session })
    }

    fn send(&mut self, method: &str, params: Value) {
        // A write error means kak is gone; the reader thread reports that.
        let _ = self.stdin.write_all(rpc::request(method, params).as_bytes());
        let _ = self.stdin.flush();
    }

    pub fn keys(&mut self, keys: &[String]) {
        if !keys.is_empty() {
            self.send("keys", json!(keys));
        }
    }

    pub fn paste(&mut self, text: &str) {
        self.send("paste", json!([text]));
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.send("resize", json!([rows, cols]));
    }

    pub fn scroll(&mut self, amount: i32, line: u16, col: u16) {
        self.send("scroll", json!([amount, line, col]));
    }

    pub fn mouse_move(&mut self, line: u16, col: u16) {
        self.send("mouse_move", json!([line, col]));
    }

    pub fn mouse_press(&mut self, button: &str, line: u16, col: u16) {
        self.send("mouse_press", json!([button, line, col]));
    }

    pub fn mouse_release(&mut self, button: &str, line: u16, col: u16) {
        self.send("mouse_release", json!([button, line, col]));
    }
}

impl KakClient {
    /// Asks the whole session to exit. Kakoune still runs its KakEnd hooks
    /// (kak-lsp shuts its language servers down there), so the process must
    /// then be left to finish: see [`KakClient::finish`].
    pub fn kill_session(&mut self) {
        if self.running() {
            session::send_command(&self.session, "kill!");
        }
    }

    /// Never kill this process on drop (it hosts the session server, or is
    /// shutting down on its own).
    pub fn detach(&mut self) {
        self.detached = true;
    }

    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Waits up to `timeout` for a process that is exiting by itself, then
    /// leaves it to finish rather than killing it. Killing kak during its
    /// KakEnd hooks orphans kak-lsp and its language servers.
    pub fn finish(mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while self.running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        self.detached = self.running();
    }
}

impl Drop for KakClient {
    fn drop(&mut self) {
        if self.detached {
            return;
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
