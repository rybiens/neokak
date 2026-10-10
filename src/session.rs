//! Per-project sessions: the files open in a project directory and the
//! cursor position in each, saved as you work and restored on request.
//!
//! Sessions live in `$XDG_STATE_HOME/neokak/sessions/`, one JSON file per
//! project directory.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::kak::session::quote;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// The project directory.
    pub cwd: String,
    /// The file that was showing.
    pub current: Option<String>,
    pub buffers: Vec<Entry>,
}

/// Where sessions are stored.
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .map(|d| d.join("neokak/sessions"))
}

/// The session file for a project directory (`/a/b` → `%a%b.json`).
pub fn file_for(dir: &Path, cwd: &Path) -> PathBuf {
    dir.join(format!("{}.json", cwd.display().to_string().replace('/', "%")))
}

/// Files that belong in a session: real files, minus temporary copies (such
/// as sudoedit's) from outside the project.
fn worth_keeping(path: &str, cwd: &Path) -> bool {
    let p = Path::new(path);
    let temp = [std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
    p.is_absolute() && p.is_file() && (p.starts_with(cwd) || !temp.iter().any(|t| p.starts_with(t)))
}

impl Session {
    /// Builds a session from `neokak-session-sync`'s arguments: pairs of
    /// file path and `line.column`.
    pub fn from_sync(cwd: &Path, current: Option<&Path>, args: &[String]) -> Self {
        let mut buffers: Vec<Entry> = Vec::new();
        for [path, pos] in args.as_chunks::<2>().0 {
            if !worth_keeping(path, cwd) || buffers.iter().any(|b| b.path == *path) {
                continue;
            }
            let (line, column) = pos.split_once('.').unwrap_or((pos, "1"));
            buffers.push(Entry {
                path: path.clone(),
                line: line.parse().unwrap_or(1).max(1),
                column: column.parse().unwrap_or(1).max(1),
            });
        }
        let current = current.map(|c| c.display().to_string()).filter(|c| buffers.iter().any(|b| b.path == *c));
        Self { cwd: cwd.display().to_string(), current, buffers }
    }

    pub fn load(file: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
    }

    /// Writes the session atomically (a crash never leaves half a file).
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, file)
    }

    /// Kakoune commands that reopen the session's files (skipping any that
    /// no longer exist), ending on the file that was showing.
    pub fn restore_commands(&self) -> Vec<String> {
        let open = |e: &Entry| format!("edit -existing -- {} {} {}", quote(&e.path), e.line, e.column);
        let existing: Vec<&Entry> = self.buffers.iter().filter(|e| Path::new(&e.path).is_file()).collect();
        let current = self.current.as_deref();
        let mut cmds: Vec<String> =
            existing.iter().filter(|e| Some(e.path.as_str()) != current).map(|e| open(e)).collect();
        if let Some(e) = existing.iter().find(|e| Some(e.path.as_str()) == current) {
            cmds.push(open(e));
        }
        cmds
    }
}

/// Project directories with a saved session, most recently used first.
pub fn projects(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(SystemTime, String)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            let session = Session::load(&e.path())?;
            let mtime = e.metadata().and_then(|m| m.modified()).ok()?;
            Path::new(&session.cwd).is_dir().then_some((mtime, session.cwd))
        })
        .collect();
    found.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
    found.into_iter().map(|(_, cwd)| cwd).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &Path, names: &[&str]) -> Vec<String> {
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, "x\n").unwrap();
                p.display().to_string()
            })
            .collect()
    }

    #[test]
    fn file_names_encode_the_directory() {
        assert_eq!(file_for(Path::new("/s"), Path::new("/home/me/proj")), Path::new("/s/%home%me%proj.json"));
    }

    #[test]
    fn sync_keeps_real_files_with_positions() {
        let root = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/target")).unwrap();
        let f = files(root.path(), &["a.rs", "b.rs"]);
        let args = [
            f[0].clone(),
            "12.4".into(),
            "*scratch*".into(),
            "1.1".into(),
            "/no/such/file.rs".into(),
            "3.3".into(),
            f[1].clone(),
            "7".into(),
            f[0].clone(),
            "99.9".into(),
        ];
        let s = Session::from_sync(root.path(), Some(Path::new(&f[1])), &args);
        assert_eq!(
            s.buffers,
            [Entry { path: f[0].clone(), line: 12, column: 4 }, Entry { path: f[1].clone(), line: 7, column: 1 }]
        );
        assert_eq!(s.current.as_deref(), Some(f[1].as_str()));
        let none = Session::from_sync(root.path(), Some(Path::new("/elsewhere")), &args);
        assert_eq!(none.current, None, "current must be one of the buffers");
    }

    #[test]
    fn temporary_files_from_outside_the_project_are_left_out() {
        let tmp = tempfile::tempdir().unwrap(); // under the system temp dir
        let f = files(tmp.path(), &["sudoedit-copy"]);
        let args = [f[0].clone(), "1.1".into()];
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(Session::from_sync(project, None, &args).buffers.is_empty());
        // A project that itself lives in a temp dir keeps its files.
        assert_eq!(Session::from_sync(tmp.path(), None, &args).buffers.len(), 1);
    }

    #[test]
    fn save_load_and_restore() {
        let root = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/target")).unwrap();
        let f = files(root.path(), &["it's.rs", "main.rs"]);
        let session = Session {
            cwd: root.path().display().to_string(),
            current: Some(f[0].clone()),
            buffers: vec![
                Entry { path: f[0].clone(), line: 3, column: 2 },
                Entry { path: f[1].clone(), line: 1, column: 1 },
                Entry { path: "/gone.rs".into(), line: 1, column: 1 },
            ],
        };
        let store = root.path().join("sessions/x.json");
        session.save(&store).unwrap();
        assert_eq!(Session::load(&store), Some(session.clone()));

        let cmds = session.restore_commands();
        assert_eq!(cmds.len(), 2, "missing files are skipped");
        assert_eq!(cmds[0], format!("edit -existing -- '{}' 1 1", f[1]));
        assert_eq!(cmds[1], format!("edit -existing -- '{}' 3 2", f[0].replace('\'', "''")), "current file last");
    }

    #[test]
    fn projects_are_listed_newest_first() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("sessions");
        for (i, name) in ["old", "new"].iter().enumerate() {
            let proj = root.path().join(name);
            std::fs::create_dir(&proj).unwrap();
            let s = Session { cwd: proj.display().to_string(), current: None, buffers: Vec::new() };
            s.save(&file_for(&store, &proj)).unwrap();
            if i == 0 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        let gone = Session { cwd: "/no/such/project".into(), current: None, buffers: Vec::new() };
        gone.save(&store.join("gone.json")).unwrap();
        let list = projects(&store);
        assert_eq!(list.len(), 2, "projects that no longer exist are hidden");
        assert!(list[0].ends_with("new") && list[1].ends_with("old"), "{list:?}");
    }
}
