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

#[test]
fn floating_terminal_answers_queries() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" t");
    nk.wait_text("Terminal");
    // A device-attributes query, as fish sends at startup: the reply
    // arrives as input.
    nk.send("stty -echo -icanon; printf '\\033[0c'; head -c 9 | od -An -c | tr -d ' '; stty sane\r");
    nk.wait_text("033[?62;22c");
    nk.send("\x1c");
    quit(nk, &sandbox);
}

#[test]
fn surround_adds_replaces_and_deletes_pairs() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send("w"); // select "hello "
    nk.send("H"); // minus the space
    nk.send(" sa(");
    nk.wait_text("(hello) from neokak");
    nk.send(" sr([");
    nk.wait_text("[hello] from neokak");
    nk.send(" sd]");
    nk.wait_text("hello from neokak");
    nk.send(" sat");
    nk.wait_text("tag:");
    nk.send("b\r");
    nk.wait_text("<b>hello</b> from neokak");
    quit(nk, &sandbox);
}

#[test]
fn space_b_number_switches_buffers() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(":edit other.txt\r");
    nk.wait_for("numbered tabs", |t| {
        let tabs = t.lines().next().unwrap_or_default();
        tabs.contains("1 notes.txt") && tabs.contains("2 other.txt") && t.contains("second file")
    });
    nk.send(" b1");
    nk.wait_text("hello from neokak");
    nk.send(" b2");
    nk.wait_text("second file");
    nk.send(" b7");
    nk.wait_text("No buffer 7");
    quit(nk, &sandbox);
}

fn write_config(sandbox: &Sandbox, text: &str) -> std::path::PathBuf {
    let dir = sandbox.home.join(".config/neokak");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("config.toml");
    std::fs::write(&file, text).unwrap();
    file
}

#[test]
fn config_file_sets_keys_and_reloads_live() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let file = write_config(&sandbox, "[keys]\nX = \"neokak-msg notify info first-binding\"\n");
    let nk = open_notes(&sandbox);
    nk.send(" X");
    nk.wait_text("first-binding");
    // Saved from anywhere: applied without restarting.
    std::fs::write(&file, "[keys]\nX = \"neokak-msg notify info second-binding\"\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1200));
    nk.send(" X");
    nk.wait_text("second-binding");
    // A broken file is reported and the last good config stays.
    std::fs::write(&file, "[keys\n").unwrap();
    nk.wait_text("config.toml:");
    quit(nk, &sandbox);
}

#[test]
fn config_file_sets_the_terminal_shell() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let shell = sandbox.home.join("myshell");
    std::fs::write(&shell, "#!/bin/sh\necho custom-shell-started\nexec /bin/sh\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
    write_config(&sandbox, &format!("[terminal]\nshell = \"{}\"\n", shell.display()));
    let nk = open_notes(&sandbox);
    nk.send(" t");
    nk.wait_text("custom-shell-started");
    nk.send("\x1c");
    quit(nk, &sandbox);
}

#[test]
fn settings_screen_edits_the_file_and_follows_it() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let file = write_config(&sandbox, "# my settings\n");
    let nk = open_notes(&sandbox);
    nk.send(" o");
    nk.wait_for("settings screen", |t| t.contains("Settings") && t.contains("shell"));
    // The first row is terminal.shell.
    nk.send("\r");
    nk.send("zsh -l\r");
    let text = common::wait_file(&file, |t| t.contains("shell = \"zsh -l\""));
    assert!(text.starts_with("# my settings"), "comments are kept: {text}");
    nk.wait_text("zsh -l");
    // Edited elsewhere: the screen follows.
    std::fs::write(&file, "[terminal]\nshell = \"fish\"\n").unwrap();
    nk.wait_for("screen updated", |t| t.contains("fish") && !t.contains("zsh -l"));
    nk.esc();
    nk.wait_for("closed", |t| !t.contains("Settings"));
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
    let count = format!("1/{}", neokak::ui::theme::THEMES.len());
    nk.wait_for("filtered", |t| t.contains(&count));
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

/// Makes the sandbox project a git repo with everything committed.
fn git_init(sandbox: &Sandbox) {
    for args in [&["init", "-q"][..], &["add", "."], &["commit", "-qm", "initial import"]] {
        let ok = std::process::Command::new("git")
            .args(["-c", "user.name=Test", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(&sandbox.project)
            .env("HOME", &sandbox.home)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }
}

#[test]
fn git_markers_blame_and_changed_files() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    git_init(&sandbox);
    let nk = open_notes(&sandbox);

    // Inline blame on a committed, unmodified line.
    nk.send(" gb");
    nk.send("j");
    nk.wait_text("Test, just now • initial import");
    nk.send(" gb");
    nk.wait_for("blame off", |t| !t.contains("initial import"));

    // A live change marker for an unsaved edit.
    nk.send("ggOadded line");
    nk.esc();
    nk.wait_for("change marker", |t| t.lines().any(|l| l.starts_with('▎') && l.contains("added line")));

    // Saved changes show up in the changed-files picker.
    nk.send(":w\r");
    nk.send(" gs");
    nk.wait_for("changed files", |t| t.contains("Changed Files") && t.contains("1/1") && t.contains("notes.txt"));
    nk.esc();
    quit(nk, &sandbox);
}

#[test]
fn programs_run_in_a_float_until_they_exit() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(":neokak-float sh -c 'echo FLOAT-OK; read line'\r");
    nk.wait_for("float", |t| t.contains("FLOAT-OK") && t.contains(" sh "));
    nk.send("\r");
    nk.wait_for("float closed", |t| !t.contains("FLOAT-OK"));

    nk.send(":neokak-float no-such-program-xyz\r");
    nk.wait_text("no-such-program-xyz not found");
    quit(nk, &sandbox);
}

#[test]
fn kakoune_theme_follows_the_colorscheme() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    let (row, col) = PILL;
    nk.send(":set-face global function rgb:123456\r");
    nk.send(":neokak-theme kakoune\r");
    nk.wait_bg("pill in the function color", row, col, vt100::Color::Rgb(0x12, 0x34, 0x56));
    // Changing the colorscheme updates the interface live.
    nk.send(":set-face global function rgb:abcdef\r");
    nk.wait_bg("new function color", row, col, vt100::Color::Rgb(0xab, 0xcd, 0xef));
    quit(nk, &sandbox);
}

/// The statusline row.
fn status(screen: &str) -> &str {
    screen.lines().nth(Neokak::ROWS as usize - 2).unwrap_or_default()
}

#[test]
fn explorer_focus_is_visible_and_the_leader_works_from_it() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" e");
    nk.wait_for("explorer focused", |t| t.contains("File Explorer") && status(t).contains("EXPLORER"));

    // <esc> goes back to the editor and leaves the explorer open.
    nk.esc();
    nk.wait_for("editor focused", |t| t.contains("File Explorer") && status(t).contains("NORMAL"));
    nk.send(" e");
    nk.wait_for("explorer focused again", |t| status(t).contains("EXPLORER"));

    // Leader mappings work from the explorer; focus comes back after.
    nk.send(" ff");
    nk.wait_text("Find Files");
    nk.esc();
    nk.wait_for("back in the explorer", |t| !t.contains("Find Files") && status(t).contains("EXPLORER"));

    // <space>e from the explorer hides it.
    nk.send(" e");
    nk.wait_for("explorer hidden", |t| !t.contains("File Explorer") && status(t).contains("NORMAL"));
    quit(nk, &sandbox);
}

#[test]
fn splits_show_a_title_bar_per_pane() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(" |");
    nk.wait_for("a winbar over each pane", |t| t.lines().nth(1).is_some_and(|l| l.matches("notes.txt").count() == 2));
    nk.send(" wq");
    nk.wait_for("no winbar with one pane", |t| {
        t.matches("hello from neokak").count() == 1 && !t.lines().nth(1).unwrap_or_default().contains("notes.txt")
    });
    quit(nk, &sandbox);
}

/// Configures a slow KakEnd hook that creates a marker file when it finishes.
fn slow_kakend_hook(sandbox: &Sandbox) -> std::path::PathBuf {
    let marker = sandbox.home.join("kakend-done");
    let config = sandbox.home.join(".config/neokak");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("init.kak"),
        format!("hook global KakEnd .* %{{ nop %sh{{ sleep 1; touch '{}' }} }}\n", marker.display()),
    )
    .unwrap();
    marker
}

#[test]
fn closing_the_window_lets_kakoune_finish_its_shutdown_hooks() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let marker = slow_kakend_hook(&sandbox);
    let mut nk = open_notes(&sandbox);
    nk.hangup();
    nk.wait_exit();
    common::wait_file(&marker, |_| marker.exists());
}

#[test]
fn quitting_lets_kakoune_finish_its_shutdown_hooks() {
    if !have_kak() {
        return;
    }
    // kak-lsp stops its language servers from a KakEnd hook; if neokak
    // kills Kakoune during that, the servers are orphaned.
    let sandbox = Sandbox::new();
    let marker = slow_kakend_hook(&sandbox);
    let mut nk = open_notes(&sandbox);
    nk.send(":q\r");
    assert!(nk.wait_exit());
    common::wait_file(&marker, |_| marker.exists());
}

fn sessions_dir(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.home.join(".local/state/neokak/sessions")
}

#[test]
fn sessions_are_saved_and_restored_from_the_dashboard() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    nk.send(":edit other.txt\r");
    nk.wait_text("second file");
    nk.send(":edit notes.txt\r");
    nk.wait_text("hello from neokak");
    nk.send("j");
    let mut nk = nk;
    nk.send(":q\r");
    assert!(nk.wait_exit());

    let file = neokak::session::file_for(&sessions_dir(&sandbox), &sandbox.project);
    let saved = neokak::session::Session::load(&file).expect("session saved");
    assert_eq!(saved.buffers.len(), 2, "{saved:?}");
    assert!(saved.current.as_deref().is_some_and(|c| c.ends_with("notes.txt")));

    // A fresh start without files: restore from the dashboard.
    let nk = Neokak::spawn(&sandbox, &["--clean"]);
    nk.wait_text("Restore session");
    nk.send("s");
    nk.wait_for("both files back, cursor on line 2", |t| {
        let tabs = t.lines().next().unwrap_or_default();
        tabs.contains("notes.txt")
            && tabs.contains("other.txt")
            && t.contains("hello from neokak")
            && status(t).contains("2:1")
    });
    nk.wait_text("Restored 2 files");
    quit(nk, &sandbox);
}

#[test]
fn sessions_can_restore_automatically() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let other = sandbox.project.join("other.txt").display().to_string();
    let session = neokak::session::Session {
        cwd: sandbox.project.display().to_string(),
        current: Some(other.clone()),
        buffers: vec![neokak::session::Entry { path: other, line: 1, column: 1 }],
    };
    session.save(&neokak::session::file_for(&sessions_dir(&sandbox), &sandbox.project)).unwrap();
    let config = sandbox.home.join(".config/neokak");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("init.kak"), "set-option global neokak_session_autorestore true\n").unwrap();

    let nk = Neokak::spawn(&sandbox, &["--clean"]);
    nk.wait_for("restored without asking", |t| t.contains("second file") && !t.contains("Restore session"));
    quit(nk, &sandbox);

    // Not when files are given.
    let nk = open_notes(&sandbox);
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(!nk.contents().contains("second file"));
    quit(nk, &sandbox);
}

#[test]
fn projects_picker_switches_project() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let other = sandbox.home.join("other-project");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("a.txt"), "from the other project\n").unwrap();
    let a = other.join("a.txt").display().to_string();
    let session = neokak::session::Session {
        cwd: other.display().to_string(),
        current: Some(a.clone()),
        buffers: vec![neokak::session::Entry { path: a, line: 1, column: 1 }],
    };
    session.save(&neokak::session::file_for(&sessions_dir(&sandbox), &other)).unwrap();

    let nk = open_notes(&sandbox);
    nk.send(" qp");
    nk.wait_for("projects picker", |t| t.contains("Projects") && t.contains("other-project"));
    nk.send("\r");
    nk.wait_for("switched, and the old project's file closed", |t| {
        let tabs = t.lines().next().unwrap_or_default();
        t.contains("from the other project") && tabs.contains("a.txt") && !tabs.contains("notes.txt")
    });
    // Pickers now work in the new project.
    nk.send(" ff");
    nk.wait_for("files of the new project", |t| t.contains("Find Files") && t.contains("1/1"));
    nk.esc();
    quit(nk, &sandbox);
    // Each project's session kept its own files.
    let saved = |dir: &std::path::Path| {
        let s = neokak::session::Session::load(&neokak::session::file_for(&sessions_dir(&sandbox), dir)).unwrap();
        s.buffers.into_iter().map(|b| b.path).collect::<Vec<_>>()
    };
    assert_eq!(saved(&other), [other.join("a.txt").display().to_string()]);
    assert_eq!(saved(&sandbox.project), [sandbox.project.join("notes.txt").display().to_string()]);
}

#[test]
fn switching_projects_moves_the_terminal_and_loads_devenv() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    // A stand-in for devenv: trusts every project and adds a variable.
    let bin = sandbox.home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let fake = bin.join("devenv");
    std::fs::write(
        &fake,
        "#!/bin/sh\ncase \"$1\" in\n  hook-should-activate) pwd ;;\n  shell) shift 2; DEVENV_ROOT=\"$PWD\" GREET=from-devenv exec \"$@\" ;;\nesac\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());

    let other = sandbox.home.join("devenv-project");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("devenv.nix"), "{ }\n").unwrap();
    std::fs::write(other.join("a.txt"), "from the devenv project\n").unwrap();
    let a = other.join("a.txt").display().to_string();
    let session = neokak::session::Session {
        cwd: other.display().to_string(),
        current: Some(a.clone()),
        buffers: vec![neokak::session::Entry { path: a, line: 1, column: 1 }],
    };
    session.save(&neokak::session::file_for(&sessions_dir(&sandbox), &other)).unwrap();

    let nk = Neokak::spawn_with_env(&sandbox, &["--clean", "notes.txt"], &[("PATH", path)]);
    nk.wait_text("hello from neokak");
    // A shell open in the first project...
    nk.send(" t");
    nk.wait_text("Terminal");
    nk.send("\x1c");
    nk.wait_for("terminal hidden", |t| !t.contains("Terminal"));

    nk.send(" qp");
    nk.wait_for("projects picker", |t| t.contains("Projects") && t.contains("devenv-project"));
    nk.send("\r");
    nk.wait_text("Loaded the project environment (devenv.nix)");
    // ...starts over in the new one, with its environment.
    nk.send(" t");
    nk.wait_text("Terminal");
    nk.send("echo \"[$GREET] $PWD\"\r");
    nk.wait_text(&format!("[from-devenv] {}", other.display()));
    nk.send("\x1c");
    quit(nk, &sandbox);
}

#[test]
fn custom_env_providers_load_at_startup() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    std::fs::write(sandbox.project.join("custom.env"), "").unwrap();
    let config = sandbox.home.join(".config/neokak");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("init.kak"),
        "set-option global neokak_env_providers 'custom.env:MY_ENV=custom env -0'\n",
    )
    .unwrap();
    let nk = open_notes(&sandbox);
    nk.wait_text("Loaded the project environment (custom.env)");
    nk.send(" t");
    nk.wait_text("Terminal");
    nk.send("echo \"[$MY_ENV]\"\r");
    nk.wait_text("[custom]");
    nk.send("\x1c");
    quit(nk, &sandbox);
}

#[test]
fn switching_projects_asks_about_unsaved_changes() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let other = sandbox.home.join("other-project");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("a.txt"), "from the other project\n").unwrap();
    let a = other.join("a.txt").display().to_string();
    let session = neokak::session::Session {
        cwd: other.display().to_string(),
        current: Some(a.clone()),
        buffers: vec![neokak::session::Entry { path: a, line: 1, column: 1 }],
    };
    session.save(&neokak::session::file_for(&sessions_dir(&sandbox), &other)).unwrap();
    let switch = |nk: &Neokak| {
        nk.send(" qp");
        nk.wait_for("projects picker", |t| t.contains("Projects") && t.contains("other-project"));
        nk.send("\r");
        nk.wait_text("Unsaved changes");
    };

    let nk = open_notes(&sandbox);
    nk.send("iedited \x1b");
    nk.wait_text("edited hello");
    // Any other key stays.
    switch(&nk);
    assert!(nk.contents().contains("notes.txt"));
    nk.send("n");
    nk.wait_text("Staying in this project");
    assert!(nk.contents().contains("edited hello"));
    // w saves, then switches.
    switch(&nk);
    nk.send("w");
    nk.wait_for("switched", |t| t.contains("from the other project") && !t.contains("notes.txt"));
    let notes = sandbox.project.join("notes.txt");
    assert!(std::fs::read_to_string(&notes).unwrap().starts_with("edited hello"));
    quit(nk, &sandbox);

    // y discards.
    let nk = open_notes(&sandbox);
    nk.send("iagain \x1b");
    nk.wait_text("again edited");
    switch(&nk);
    nk.send("y");
    nk.wait_for("switched", |t| t.contains("from the other project") && !t.contains("notes.txt"));
    assert!(std::fs::read_to_string(&notes).unwrap().starts_with("edited hello"));
    quit(nk, &sandbox);
}

#[test]
fn sessions_can_be_forgotten() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let file = neokak::session::file_for(&sessions_dir(&sandbox), &sandbox.project);
    let nk = open_notes(&sandbox);
    nk.send(":edit other.txt\r");
    nk.wait_text("second file");
    common::wait_file(&file, |s| s.contains("other.txt"));
    nk.send(" qd");
    nk.wait_text("Session forgotten");
    assert!(!file.exists());
    nk.send(":edit notes.txt\r");
    nk.wait_text("hello from neokak");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!file.exists(), "not saved again this run");
    quit(nk, &sandbox);
}

#[test]
fn indented_blocks_fold_and_unfold() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    std::fs::write(
        sandbox.project.join("f.py"),
        "def outer():\n    one = 1\n    two = 2\n    if one:\n        three = 3\n\nafter = 0\n",
    )
    .unwrap();
    let nk = Neokak::spawn(&sandbox, &["--clean", "f.py"]);
    nk.wait_for("file", |t| t.contains("after = 0") && t.contains("NORMAL"));
    nk.send(" zc");
    nk.wait_for("folded", |t| t.contains("def outer(): ··· 4 lines") && !t.contains("two = 2"));
    // Moving the cursor into the fold shows its lines; leaving folds it again.
    nk.send("j");
    nk.wait_for("revealed under the cursor", |t| t.contains("two = 2"));
    nk.send("ge");
    nk.wait_for("folded again", |t| t.contains("··· 4 lines"));
    nk.send(" zR");
    nk.wait_for("unfolded", |t| t.contains("two = 2") && !t.contains("···"));
    quit(nk, &sandbox);
}

#[test]
fn references_open_with_context() {
    if !have_kak() {
        return;
    }
    let sandbox = Sandbox::new();
    let nk = open_notes(&sandbox);
    let root = sandbox.project.display().to_string();
    nk.send(&format!(
        ":neokak-lsp context:References %{{lsp-show-goto-buffer *goto* lsp-goto '{root}' 'notes.txt:2:1: line two'}}\r"
    ));
    nk.wait_for("context view", |t| {
        t.contains("*references*")
            && t.contains("notes.txt-1-hello from neokak")
            && t.contains("notes.txt:2:1:line two")
    });
    // <ret> on the reference jumps to it.
    nk.send("j");
    nk.send("\r");
    nk.wait_for("jumped", |t| status(t).contains("notes.txt") && status(t).contains("2:1"));
    quit(nk, &sandbox);
}
