//! Opt-in event log: set NEOKAK_DEBUG=1 to append to
//! ~/.local/state/neokak/debug.log.

use std::io::Write;
use std::sync::OnceLock;

static LOG: OnceLock<Option<std::sync::Mutex<std::fs::File>>> = OnceLock::new();

pub fn log(msg: std::fmt::Arguments) {
    let log = LOG.get_or_init(|| {
        std::env::var_os("NEOKAK_DEBUG")?;
        let path = crate::log_path().with_file_name("debug.log");
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok().map(std::sync::Mutex::new)
    });
    if let Some(f) = log {
        let _ = writeln!(f.lock().unwrap(), "{msg}");
    }
}

#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => { $crate::debug::log(format_args!($($t)*)) };
}
