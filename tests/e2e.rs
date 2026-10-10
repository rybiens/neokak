//! End-to-end: the real `neokak` binary in a pseudo-terminal.

mod common;

use common::{Neokak, Sandbox, have_kak, wait_file};

/// Opens notes.txt and waits until neokak has drawn it.
fn open_notes(sandbox: &Sandbox) -> Neokak {
    let nk = Neokak::spawn(sandbox, &["--clean", "notes.txt"]);
    nk.wait_for("the file, statusline and bufferline", |t| {
        t.contains("hello from neokak") && t.contains("NORMAL") && t.contains("notes.txt")
    });
    nk
}

fn quit(mut nk: Neokak, sandbox: &Sandbox) {
    nk.esc();
    nk.send(":q!\r");
    assert!(nk.wait_exit(), "neokak exited with an error");
    assert!(sandbox.neokak_dirs().is_empty(), "runtime dir left behind: {:?}", sandbox.neokak_dirs());
    assert!(sandbox.kak_sessions().is_empty(), "kak session left behind: {:?}", sandbox.kak_sessions());
}

#[test]
fn opens_a_file_and_quits_cleanly() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    quit(nk, &sandbox);
}

#[test]
fn shows_the_dashboard_without_files() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let mut nk = Neokak::spawn(&sandbox, &["--clean"]);
    nk.wait_for("dashboard", |t| t.contains("Find file") && t.contains("Recent files"));
    nk.send("q");
    assert!(nk.wait_exit());
}

#[test]
fn edits_and_saves() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send("ggiNEW ");
    nk.esc();
    nk.send(":w\r");
    wait_file(&sandbox.project.join("notes.txt"), |s| s.starts_with("NEW hello from neokak"));
    quit(nk, &sandbox);
}

#[test]
fn which_key_lists_leader_mappings() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" ");
    nk.wait_for("which-key panel", |t| t.contains("+find") && t.contains("file explorer"));
    quit(nk, &sandbox);
}

#[test]
fn file_picker_opens_the_selection() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" ff");
    nk.wait_text("Find Files");
    nk.send("other");
    nk.wait_for("filtered results", |t| t.contains("1/2"));
    nk.send("\r");
    nk.wait_text("second file");
    quit(nk, &sandbox);
}

#[test]
fn live_grep_jumps_to_the_match() {
    if !have_kak() || std::process::Command::new("rg").arg("--version").output().is_err() {
        eprintln!("skipping: needs kak and rg");
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" fg");
    nk.wait_text("Live Grep");
    nk.send("second");
    nk.wait_text("other.txt:1:1:second file");
    nk.send("\r");
    nk.wait_for("other.txt opened", |t| t.contains("second file") && !t.contains("Live Grep"));
    quit(nk, &sandbox);
}

#[test]
fn explorer_and_splits() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" e");
    nk.wait_for("explorer", |t| t.contains("File Explorer") && t.contains("other.txt"));
    nk.esc();
    nk.send(" |");
    nk.wait_for("two panes", |t| t.matches("hello from neokak").count() == 2);
    // Closing the split leaves one pane.
    nk.send(" wq");
    nk.wait_for("one pane", |t| t.matches("hello from neokak").count() == 1);
    quit(nk, &sandbox);
}

#[test]
fn floating_terminal_runs_a_shell() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" t");
    nk.wait_text("Terminal");
    nk.send("echo neo$((40 + 2))\r");
    nk.wait_text("neo42");
    nk.send("\x1c"); // <c-\> hides it
    nk.wait_for("terminal hidden", |t| !t.contains("neo42"));
    quit(nk, &sandbox);
}

/// The statusline's mode pill uses the theme's blue.
const PILL: (u16, u16) = (Neokak::ROWS - 2, 1);

#[test]
fn themes_switch_live() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    let (row, col) = PILL;
    nk.wait_bg("gruvbox blue", row, col, vt100::Color::Rgb(0x83, 0xa5, 0x98));

    nk.send(":neokak-theme tokyonight\r");
    nk.wait_bg("tokyonight blue", row, col, vt100::Color::Rgb(0x7a, 0xa2, 0xf7));

    nk.send(":set-option global neokak_theme_colors blue=#ff0000\r");
    nk.wait_bg("overridden blue", row, col, vt100::Color::Rgb(0xff, 0x00, 0x00));

    nk.send(":neokak-theme nonexistent\r");
    nk.wait_text("unknown theme");
    quit(nk, &sandbox);
}

#[test]
fn theme_picker_applies_the_choice() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" ft");
    nk.wait_for("themes picker", |t| t.contains("Themes") && t.contains("catppuccin"));
    nk.send("nord");
    nk.wait_for("filtered", |t| t.contains("1/6"));
    nk.send("\r");
    let (row, col) = PILL;
    nk.wait_bg("nord blue", row, col, vt100::Color::Rgb(0x81, 0xa1, 0xc1));
    quit(nk, &sandbox);
}

#[test]
fn untitled_info_floats_above_the_cursor() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let text: String = (1..=40).map(|i| format!("line {i}\n")).collect();
    std::fs::write(sandbox.project.join("long.txt"), text).unwrap();
    let nk = Neokak::spawn(&sandbox, &["--clean", "+20", "long.txt"]);
    let screen = nk.wait_for("long.txt at line 20", |t| t.contains("line 20") && t.contains("NORMAL"));
    let cursor_row = screen.lines().position(|l| l.contains("line 20")).unwrap();

    // `info` without -anchor or -title is what kak-lsp's hover sends.
    nk.send(":info 'fn foo(x: i32) -> bool'\r");
    let screen = nk.wait_text("fn foo(x: i32) -> bool");
    let hover_row = screen.lines().position(|l| l.contains("fn foo(x: i32) -> bool")).unwrap();
    assert!(hover_row < cursor_row, "hover at row {hover_row} should be above the cursor row {cursor_row}");
    assert!(screen.lines().nth(cursor_row).unwrap().contains("line 20"), "cursor line stays visible");
    quit(nk, &sandbox);
}
