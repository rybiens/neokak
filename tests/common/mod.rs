//! Shared helpers for the integration tests.
#![allow(dead_code)] // Each test binary uses a different subset.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Whether `kak` is available. Without it the kak-backed tests are skipped,
/// unless NEOKAK_REQUIRE_KAK is set (as in CI), where that's a failure.
pub fn have_kak() -> bool {
    let found = Command::new("kak").arg("-version").output().is_ok_and(|o| o.status.success());
    if !found {
        assert!(std::env::var_os("NEOKAK_REQUIRE_KAK").is_none(), "kak not found but NEOKAK_REQUIRE_KAK is set");
        eprintln!("skipping: kak not found on PATH");
    }
    found
}

/// A throwaway project plus isolated HOME/XDG directories, so tests never
/// read or write the real user config, state or Kakoune sessions.
pub struct Sandbox {
    _root: tempfile::TempDir,
    pub project: PathBuf,
    pub home: PathBuf,
    pub runtime: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let project = base.join("project");
        let home = base.join("home");
        let runtime = base.join("run");
        for d in [&project, &home, &runtime] {
            std::fs::create_dir_all(d).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Kakoune refuses session dirs other users can read.
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        std::fs::write(project.join("notes.txt"), "hello from neokak\nline two\n").unwrap();
        std::fs::write(project.join("other.txt"), "second file\n").unwrap();
        Self { _root: root, project, home, runtime }
    }

    pub fn env(&self) -> Vec<(&'static str, PathBuf)> {
        vec![
            ("HOME", self.home.clone()),
            ("XDG_CONFIG_HOME", self.home.join(".config")),
            ("XDG_STATE_HOME", self.home.join(".local/state")),
            ("XDG_DATA_HOME", self.home.join(".local/share")),
            ("XDG_RUNTIME_DIR", self.runtime.clone()),
        ]
    }

    /// Kakoune session sockets still present in the sandbox.
    pub fn kak_sessions(&self) -> Vec<String> {
        std::fs::read_dir(self.runtime.join("kakoune"))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default()
    }

    /// neokak's own per-process runtime dirs still present.
    pub fn neokak_dirs(&self) -> Vec<String> {
        std::fs::read_dir(self.runtime.join("neokak"))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default()
    }
}

/// The neokak binary running in a pseudo-terminal, with its screen parsed.
pub struct Neokak {
    screen: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn Child + Send + Sync>,
}

impl Neokak {
    pub const ROWS: u16 = 30;
    pub const COLS: u16 = 110;

    pub fn spawn(sandbox: &Sandbox, args: &[&str]) -> Self {
        Self::spawn_with_env(sandbox, args, &[])
    }

    /// Like [`Neokak::spawn`], with extra environment variables.
    pub fn spawn_with_env(sandbox: &Sandbox, args: &[&str], env: &[(&str, String)]) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize { rows: Self::ROWS, cols: Self::COLS, pixel_width: 0, pixel_height: 0 })
            .unwrap();
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_neokak"));
        cmd.args(args);
        cmd.cwd(&sandbox.project);
        for (k, v) in sandbox.env() {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("SHELL", "/bin/sh");
        cmd.env("NEOKAK_ASCII", "1");
        // kak-lsp daemons outlive sessions shorter than their startup.
        cmd.env("NEOKAK_NO_LSP", "1");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);

        let screen = Arc::new(Mutex::new(vt100::Parser::new(Self::ROWS, Self::COLS, 0)));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(pair.master.take_writer().unwrap()));
        let mut reader = pair.master.try_clone_reader().unwrap();
        let (screen2, writer2) = (screen.clone(), writer.clone());
        thread::spawn(move || {
            let _master = pair.master; // keep the pty open while reading
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = &buf[..n];
                // Answer the device-attributes query crossterm sends at
                // startup, so it doesn't wait for a timeout.
                if chunk.windows(3).any(|w| w == b"\x1b[c") {
                    let _ = writer2.lock().unwrap().write_all(b"\x1b[?62c");
                }
                screen2.lock().unwrap().process(chunk);
            }
        });
        Self { screen, writer, child }
    }

    pub fn contents(&self) -> String {
        self.screen.lock().unwrap().screen().contents()
    }

    /// Background color of one screen cell.
    pub fn bg_at(&self, row: u16, col: u16) -> vt100::Color {
        self.screen.lock().unwrap().screen().cell(row, col).map_or(vt100::Color::Default, |c| c.bgcolor())
    }

    /// Waits until a cell's background becomes `want`.
    pub fn wait_bg(&self, what: &str, row: u16, col: u16, want: vt100::Color) {
        let deadline = Instant::now() + TIMEOUT;
        while self.bg_at(row, col) != want {
            if Instant::now() > deadline {
                panic!("timed out waiting for {what}: cell ({row},{col}) is {:?}, want {want:?}", self.bg_at(row, col));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Waits until `pred` holds for the screen text; panics with the screen
    /// on timeout.
    pub fn wait_for(&self, what: &str, pred: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let text = self.contents();
            if pred(&text) {
                return text;
            }
            if Instant::now() > deadline {
                panic!("timed out waiting for {what}; screen:\n{text}");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn wait_text(&self, needle: &str) -> String {
        self.wait_for(&format!("{needle:?}"), |t| t.contains(needle))
    }

    /// Sends raw bytes, then pauses briefly so separate sends aren't merged
    /// into one read (an Esc followed by a key would parse as Alt+key).
    pub fn send(&self, bytes: &str) {
        let mut w = self.writer.lock().unwrap();
        w.write_all(bytes.as_bytes()).unwrap();
        w.flush().unwrap();
        drop(w);
        thread::sleep(Duration::from_millis(150));
    }

    pub fn esc(&self) {
        self.send("\x1b");
    }

    /// Sends SIGHUP to neokak's whole process group, as a terminal does when
    /// its window is closed.
    pub fn hangup(&self) {
        let pid = self.child.process_id().expect("pid");
        // The pty made neokak a session leader, so its group id is its pid.
        let ok = Command::new("kill").args(["-HUP", "--", &format!("-{pid}")]).status().unwrap().success();
        assert!(ok, "kill -HUP failed");
    }

    /// Waits for the process to exit and returns whether it succeeded.
    pub fn wait_exit(&mut self) -> bool {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }
            if Instant::now() > deadline {
                panic!("neokak did not exit; screen:\n{}", self.contents());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Neokak {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
    }
}

/// Polls until a file's contents satisfy `pred`.
pub fn wait_file(path: &Path, pred: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if pred(&text) {
            return text;
        }
        assert!(Instant::now() < deadline, "timed out waiting on {}; contents: {text:?}", path.display());
        thread::sleep(Duration::from_millis(50));
    }
}
