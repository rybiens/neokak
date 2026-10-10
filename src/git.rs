//! Git working-tree status for the file tree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ignored,
    Untracked,
    Renamed,
    Added,
    Modified,
    Deleted,
    Conflict,
}

impl Status {
    fn from_xy(xy: &str) -> Option<Self> {
        let b = xy.as_bytes();
        let (x, y) = (*b.first()? as char, *b.get(1)? as char);
        Some(match (x, y) {
            ('?', '?') => Self::Untracked,
            ('!', '!') => Self::Ignored,
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => Self::Conflict,
            ('D', _) | (_, 'D') => Self::Deleted,
            ('R', _) | (_, 'R') => Self::Renamed,
            ('A', _) => Self::Added,
            _ => Self::Modified,
        })
    }

    pub fn letter(self) -> char {
        match self {
            Self::Ignored => '!',
            Self::Untracked => 'U',
            Self::Renamed => 'R',
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Deleted => 'D',
            Self::Conflict => 'C',
        }
    }
}

#[derive(Default)]
pub struct GitStatus {
    /// Status of exact paths. Entries for directories (from `dir/` lines)
    /// apply to everything below them.
    paths: HashMap<PathBuf, Status>,
    /// Directories containing changed files, with the most severe status.
    dirs: HashMap<PathBuf, Status>,
}

impl GitStatus {
    pub fn load(dir: &Path) -> Self {
        let mut st = Self::default();
        let Some(root) = git(dir, &["rev-parse", "--show-toplevel"]) else { return st };
        let root = PathBuf::from(root.trim_end());
        let Some(out) = git(dir, &["status", "--porcelain=v1", "-z", "--ignored=matching"]) else { return st };
        let mut records = out.split('\0');
        while let Some(rec) = records.next() {
            let (Some(xy), Some(path)) = (rec.get(..2), rec.get(3..)) else { continue };
            if xy.starts_with('R') || xy.starts_with('C') {
                records.next(); // rename source
            }
            let Some(status) = Status::from_xy(xy) else { continue };
            let path = root.join(path.trim_end_matches('/'));
            if status != Status::Ignored {
                for parent in path.ancestors().skip(1) {
                    if parent.starts_with(&root) {
                        let e = st.dirs.entry(parent.to_path_buf()).or_insert(status);
                        *e = (*e).max(status);
                    }
                }
            }
            st.paths.insert(path, status);
        }
        st
    }

    pub fn get(&self, path: &Path) -> Option<Status> {
        if let Some(s) = self.paths.get(path).or_else(|| self.dirs.get(path)) {
            return Some(*s);
        }
        // Inside an untracked or ignored directory?
        path.ancestors().skip(1).find_map(|p| self.paths.get(p).copied())
    }
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    #[test]
    fn reads_working_tree_status() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        git(&root, &["init", "-q"]);
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/tracked.rs"), "a").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "init"]);

        std::fs::write(root.join("src/tracked.rs"), "b").unwrap();
        std::fs::write(root.join("new.txt"), "").unwrap();
        std::fs::create_dir(root.join("target")).unwrap();
        std::fs::write(root.join("target/out"), "").unwrap();

        let st = GitStatus::load(&root);
        assert_eq!(st.get(&root.join("src/tracked.rs")), Some(Status::Modified));
        assert_eq!(st.get(&root.join("src")), Some(Status::Modified), "parent dirs inherit changes");
        assert_eq!(st.get(&root.join("new.txt")), Some(Status::Untracked));
        assert_eq!(st.get(&root.join("target/out")), Some(Status::Ignored), "inside an ignored dir");
        assert_eq!(st.get(&root.join(".gitignore")), None);
    }

    #[test]
    fn outside_a_repo_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), "").unwrap();
        assert_eq!(GitStatus::load(dir.path()).get(&dir.path().join("f")), None);
    }

    #[test]
    fn xy_codes() {
        assert_eq!(Status::from_xy("??"), Some(Status::Untracked));
        assert_eq!(Status::from_xy("!!"), Some(Status::Ignored));
        assert_eq!(Status::from_xy(" M"), Some(Status::Modified));
        assert_eq!(Status::from_xy("A "), Some(Status::Added));
        assert_eq!(Status::from_xy("UU"), Some(Status::Conflict));
    }
}
