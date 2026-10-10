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

/// Column of the first `│` on a screen row (pane or explorer separator).
fn separator_col(screen: &str, row: usize) -> Option<usize> {
    screen.lines().nth(row)?.chars().position(|c| c == '│')
}

/// SGR mouse report (1-based coordinates), as terminals send them.
fn mouse(kind: u8, col: u16, row: u16, release: bool) -> String {
    format!("\x1b[<{kind};{};{}{}", col + 1, row + 1, if release { 'm' } else { 'M' })
}

#[test]
fn splits_resize_by_keyboard_and_mouse() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" |");
    let screen = nk.wait_for("two panes", |t| t.matches("hello from neokak").count() == 2);
    let start = separator_col(&screen, 5).expect("separator");
    // 109 columns besides the separator; the first pane gets the odd one.
    assert_eq!(start, Neokak::COLS as usize / 2);

    // The new right pane has focus, so growing it moves the separator left.
    nk.send("\x1b[1;5C"); // Ctrl+Right
    nk.wait_for("right pane wider", |t| separator_col(t, 5) == Some(start - 2));
    nk.send(" w=");
    nk.wait_for("equal again", |t| separator_col(t, 5) == Some(start));

    // Drag the separator to column 30.
    let row = 5;
    nk.send(&mouse(0, start as u16, row, false));
    nk.send(&mouse(32, 30, row, false));
    nk.send(&mouse(0, 30, row, true));
    nk.wait_for("separator dragged", |t| separator_col(t, 5) == Some(30));
    nk.send(" wq");
    nk.wait_for("one pane", |t| t.matches("hello from neokak").count() == 1);
    quit(nk, &sandbox);
}

#[test]
fn explorer_width_changes() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" e");
    let screen = nk.wait_for("explorer", |t| t.contains("File Explorer") && t.contains("other.txt"));
    let edge = separator_col(&screen, 3).expect("explorer edge");
    assert_eq!(edge, 31, "default width 32");
    nk.send(">>");
    nk.wait_for("wider explorer", |t| separator_col(t, 3) == Some(35));
    // Drag its edge.
    nk.send(&mouse(0, 35, 3, false));
    nk.send(&mouse(32, 20, 3, false));
    nk.send(&mouse(0, 20, 3, true));
    nk.wait_for("dragged edge", |t| separator_col(t, 3) == Some(20));
    quit(nk, &sandbox);
}

#[test]
fn lsp_result_lists_open_in_a_picker() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    // What kak-lsp calls to show a result list (here without kak-lsp).
    let root = sandbox.project.display().to_string();
    nk.send(&format!(
        ":neokak-lsp References %{{lsp-show-goto-buffer *goto* lsp-goto '{root}' 'notes.txt:2:1: line two'}}\r"
    ));
    nk.wait_for("references picker", |t| t.contains("References") && t.contains("notes.txt:2:1: line two"));
    nk.send("\r");
    nk.wait_for("cursor on line 2", |t| t.lines().any(|l| l.contains("NORMAL") && l.contains("2:1")));

    // An empty list is a notification, not an empty picker.
    nk.send(&format!(":lsp-show-goto-buffer *diagnostics* lsp-goto '{root}' ''\r"));
    nk.wait_text("Diagnostics: nothing found");
    quit(nk, &sandbox);
}
