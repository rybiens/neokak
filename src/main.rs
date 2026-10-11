use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, bail};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use neokak::event::AppEvent;
use neokak::kak::client::KakClient;
use neokak::kak::session::quote;
use neokak::ui::icons::Icons;
use neokak::{RC, app, ipc, log_path};

static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);

const USAGE: &str = "\
neokak - a Neovim-style IDE frontend for Kakoune

Usage: neokak [options] [files...]

Options:
  --clean      ignore ~/.config/kak (load only neokak's defaults)
  --ascii      no Nerd Font glyphs
  -h, --help   show this help
  -V, --version
Other arguments are passed through to kak.

Config: ~/.config/neokak/init.kak is sourced after neokak's defaults.";

struct Options {
    clean: bool,
    ascii: bool,
    kak_args: Vec<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "msg") {
        let sock = std::env::var_os("NEOKAK_SOCK").context("NEOKAK_SOCK not set (not inside neokak?)")?;
        return ipc::send(&PathBuf::from(sock), &args[1..]);
    }

    let mut opts = Options { clean: false, ascii: std::env::var_os("NEOKAK_ASCII").is_some(), kak_args: Vec::new() };
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("neokak {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--clean" => opts.clean = true,
            "--ascii" => opts.ascii = true,
            _ => opts.kak_args.push(arg),
        }
    }
    run(opts)
}

fn runtime_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("neokak"))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("neokak-{}", std::env::var("USER").unwrap_or_default())));
    base.join(std::process::id().to_string())
}

fn run(opts: Options) -> Result<()> {
    if !io::IsTerminal::is_terminal(&io::stdout()) {
        bail!("neokak needs a terminal");
    }
    let dir = runtime_dir();
    sweep_stale(dir.parent().unwrap());
    std::fs::create_dir_all(&dir)?;
    let _cleanup = Cleanup(dir.clone());
    run_in(&dir, opts)
}

/// Removes the runtime dir on every exit path, including unwinding.
struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        // Drop session sockets left by kak processes that died uncleanly.
        let _ = Command::new("kak").arg("-clear").stderr(std::process::Stdio::null()).status();
    }
}

/// Removes runtime dirs of neokak processes that are no longer running.
fn sweep_stale(base: &std::path::Path) {
    // Liveness is checked through /proc; without it, leave everything alone.
    if !std::path::Path::new("/proc/self").exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(base) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn run_in(dir: &std::path::Path, opts: Options) -> Result<()> {
    let rc = dir.join("neokak.kak");
    std::fs::write(&rc, RC)?;
    let sock = dir.join("ipc.sock");
    let session = format!("neokak-{}", std::process::id());

    let (tx, rx) = mpsc::channel();
    ipc::listen(&sock, tx.clone())?;

    // config.toml, as a kakscript neokak.kak sources before init.kak.
    let config_path = neokak::config::path();
    let (config, mut config_problems) = match config_path.as_deref().map(neokak::config::Config::load) {
        Some(Ok(c)) => {
            let problems = c.problems();
            (c, problems)
        }
        Some(Err(e)) => (neokak::config::Config::default(), vec![e]),
        None => (neokak::config::Config::default(), Vec::new()),
    };
    let (script, applied) = config.kakscript(&neokak::config::Applied::default());
    let config_kak = dir.join("config.kak");
    if let Err(e) = std::fs::write(&config_kak, script) {
        config_problems.push(format!("{}: {e}", config_kak.display()));
    }

    let mut cmd = Command::new("kak");
    cmd.args(["-s", &session, "-ui", "json", "-e"])
        .arg(format!("rename-client {}; source {}", app::MAIN_CLIENT, quote(&rc.display().to_string())))
        .args(&opts.kak_args)
        .env("NEOKAK_BIN", std::env::current_exe()?)
        .env("NEOKAK_SOCK", &sock)
        .env("NEOKAK_CONFIG_KAK", &config_kak);
    if opts.clean {
        let empty = dir.join("config");
        std::fs::create_dir_all(&empty)?;
        cmd.env("KAKOUNE_CONFIG_DIR", empty);
    }
    let log = log_path();
    let _ = std::fs::remove_file(&log);
    let kak = KakClient::spawn(cmd, session.clone(), &log, 0, tx.clone())?;

    let mut signals = signal_hook::iterator::Signals::new([signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM])?;
    let signal_tx = tx.clone();
    thread::spawn(move || {
        if signals.forever().next().is_some() {
            let _ = signal_tx.send(AppEvent::Quit);
        }
    });

    let mut terminal = setup_terminal()?;
    let input_tx = tx.clone();
    thread::spawn(move || {
        while let Ok(ev) = crossterm::event::read() {
            if input_tx.send(AppEvent::Term(ev)).is_err() {
                return;
            }
        }
        // The tty is gone; don't linger as an orphan even if SIGHUP was missed.
        let _ = input_tx.send(AppEvent::Quit);
    });

    let has_files = opts.kak_args.iter().any(|a| !a.starts_with(['-', '+']));
    let info = app::SessionInfo { name: session, log, dir: dir.to_path_buf() };
    let mut app = app::App::new(kak, info, tx, Icons { nerd: !opts.ascii }, !has_files);
    app.set_config(config_path, config, applied, config_problems);
    let result = app.run(&mut terminal, rx);
    restore_terminal();
    // Terminal's Drop restores the cursor itself and eprintln!s on failure,
    // which panics once the tty is gone; we've already restored everything.
    std::mem::forget(terminal);
    // Now that the terminal is back, wait for Kakoune to finish shutting down.
    result.map(drop)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste, cursor::Hide)?;
    if terminal::supports_keyboard_enhancement().unwrap_or(false) {
        execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))?;
        KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

fn restore_terminal() {
    let mut out = io::stdout();
    // Not re-queried here: the input thread would swallow the reply.
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();
}
