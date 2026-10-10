//! Most-recently-opened files, persisted across sessions.

use std::path::PathBuf;

const MAX: usize = 200;

pub struct Recent {
    path: Option<PathBuf>,
    pub files: Vec<String>,
}

impl Recent {
    pub fn load() -> Self {
        let path = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .map(|d| d.join("neokak/recent"));
        Self::at(path)
    }

    /// Loads (and later saves) the list at `path`; `None` keeps it in memory.
    pub fn at(path: Option<PathBuf>) -> Self {
        let files = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.lines().map(str::to_owned).collect())
            .unwrap_or_default();
        Self { path, files }
    }

    /// Moves `file` to the front if it is a real file on disk.
    pub fn touch(&mut self, file: &str) {
        if !file.starts_with('/') || !std::path::Path::new(file).is_file() {
            return;
        }
        if self.files.first().is_some_and(|f| f == file) {
            return;
        }
        self.files.retain(|f| f != file);
        self.files.insert(0, file.to_owned());
        self.files.truncate(MAX);
        if let Some(p) = &self.path {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.files.join("\n") + "\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_orders_dedupes_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("state/recent");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        let (a, b) = (a.display().to_string(), b.display().to_string());

        let mut r = Recent::at(Some(store.clone()));
        r.touch(&a);
        r.touch(&b);
        r.touch(&a);
        assert_eq!(r.files, [a.clone(), b.clone()]);

        assert_eq!(Recent::at(Some(store)).files, [a, b]);
    }

    #[test]
    fn ignores_relative_and_missing_paths() {
        let mut r = Recent::at(None);
        r.touch("relative.txt");
        r.touch("/definitely/not/here.txt");
        r.touch("*scratch*");
        assert!(r.files.is_empty());
    }
}
