//! neokak's Kakoune client against a real `kak -ui json`.

mod common;

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use common::{Sandbox, TIMEOUT, have_kak};
use neokak::event::AppEvent;
use neokak::kak::client::KakClient;
use neokak::kak::rpc::{KakEvent, line_text};

struct Kak {
    client: KakClient,
    rx: Receiver<AppEvent>,
    _sandbox: Sandbox,
}

fn start() -> Option<Kak> {
    if !have_kak() {
        return None;
    }
    let sandbox = Sandbox::new();
    let session = format!("neokak-test-{}", std::process::id());
    let mut cmd = Command::new("kak");
    cmd.args(["-n", "-s", &session, "-ui", "json"]).arg(sandbox.project.join("notes.txt"));
    for (k, v) in sandbox.env() {
        cmd.env(k, v);
    }
    let (tx, rx) = mpsc::channel();
    let mut client = KakClient::spawn(cmd, session, &sandbox.home.join("kak.log"), 0, tx).unwrap();
    client.resize(10, 60);
    Some(Kak { client, rx, _sandbox: sandbox })
}

impl Kak {
    /// Waits for an event matching `pred`.
    fn expect(&self, what: &str, mut pred: impl FnMut(&KakEvent) -> bool) -> KakEvent {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(AppEvent::Kak(_, ev)) if pred(&ev) => return ev,
                Ok(AppEvent::KakExited(_)) => panic!("kak exited while waiting for {what}"),
                Ok(_) => {}
                Err(_) => panic!("timed out waiting for {what}"),
            }
        }
    }

    fn expect_draw(&self, needle: &str) -> Vec<String> {
        let KakEvent::Draw { lines, .. } = self.expect(needle, |ev| match ev {
            KakEvent::Draw { lines, .. } => lines.iter().any(|l| line_text(l).contains(needle)),
            _ => false,
        }) else {
            unreachable!()
        };
        lines.iter().map(line_text).collect()
    }

    fn keys(&mut self, keys: &[&str]) {
        self.client.keys(&keys.iter().map(|k| k.to_string()).collect::<Vec<_>>());
    }
}

#[test]
fn draws_the_buffer() {
    let Some(kak) = start() else { return };
    let lines = kak.expect_draw("hello from neokak");
    assert!(lines[1].starts_with("line two"), "{lines:?}");
}

#[test]
fn keys_edit_the_buffer() {
    let Some(mut kak) = start() else { return };
    kak.expect_draw("hello from neokak");
    kak.keys(&["i", "X", "Y", "<esc>"]);
    kak.expect_draw("XYhello from neokak");
}

#[test]
fn prompt_reports_status_and_completion_menu() {
    let Some(mut kak) = start() else { return };
    kak.expect_draw("hello");
    kak.keys(&[":", "e", "d"]);
    // The status and the completion menu can arrive in either order.
    let (mut status, mut menu) = (false, false);
    kak.expect("command prompt and completion menu", |ev| {
        match ev {
            // The prompt's cursor is drawn as a trailing atom.
            KakEvent::DrawStatus { style, content, .. } => {
                status |= style == "command" && line_text(content).trim_end() == "ed";
            }
            KakEvent::MenuShow { items, style, .. } => {
                menu |= style == "prompt" && items.iter().any(|i| line_text(i) == "edit");
            }
            _ => {}
        }
        status && menu
    });
}

#[test]
fn quitting_ends_the_client() {
    let Some(mut kak) = start() else { return };
    kak.expect_draw("hello");
    kak.keys(&[":", "q", "!", "<ret>"]);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match kak.rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(AppEvent::KakExited(0)) => break,
            Ok(_) => {}
            Err(_) => panic!("kak did not exit"),
        }
    }
}
