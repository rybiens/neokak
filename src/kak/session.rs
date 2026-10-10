//! Talking to the Kakoune session out of band (`kak -p`).

use std::io::Write;
use std::process::{Command, Stdio};

/// Quotes `s` as a single Kakoune word.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Sends `cmd` to the session, blocking until `kak -p` has delivered it.
pub fn send_command(session: &str, cmd: &str) {
    let Ok(mut child) = Command::new("kak")
        .args(["-p", session])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(cmd.as_bytes());
    }
    let _ = child.wait();
}

/// Evaluates `cmd` in the context of `client`, without blocking the caller.
pub fn eval_in_client(session: &str, client: &str, cmd: &str) {
    let full = format!("evaluate-commands -client {} {}", quote(client), quote(cmd));
    let session = session.to_owned();
    std::thread::spawn(move || send_command(&session, &full));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes() {
        assert_eq!(quote("it's"), "'it''s'");
        assert_eq!(quote("plain"), "'plain'");
    }
}
