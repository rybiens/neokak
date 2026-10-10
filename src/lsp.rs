//! Turning kak-lsp's result lists into picker items.
//!
//! kak-lsp shows symbols, references, diagnostics and similar lists through
//! its `lsp-show-goto-buffer` command (buffer name, filetype, project root,
//! content). `rc/neokak.kak` overrides that command to send the results here
//! instead. Each content line is `path:line:col:text`, with paths relative to
//! the project root. In document-symbol lists the first line names the file
//! and `%` stands for it.

use std::path::Path;

use crate::ui::picker::{location, parse_grep, parse_location};

/// Picker title for a kak-lsp result buffer.
pub fn title(bufname: &str, filetype: &str) -> String {
    match (bufname, filetype) {
        (_, "lsp-document-symbol") => "Document Symbols".into(),
        ("*symbols*", _) => "Workspace Symbols".into(),
        ("*goto*", _) => "Locations".into(),
        (name, _) => {
            let name = name.trim_matches('*');
            let mut chars = name.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().chain(chars).collect(),
                None => "LSP".into(),
            }
        }
    }
}

/// Location items (see [`crate::ui::picker::Accept::Location`]) for a
/// result list.
pub fn items(filetype: &str, root: &Path, content: &str) -> Vec<String> {
    let mut lines = content.lines();
    let symbols = filetype == "lsp-document-symbol";
    let current = if symbols { lines.next().unwrap_or_default().trim() } else { "" };
    lines
        .filter_map(|line| {
            let (path, row, col, text) = parse_grep(line)?;
            let path = if path == "%" { current } else { path };
            let abs = if Path::new(path).is_absolute() { path.into() } else { root.join(path) };
            let label = if symbols {
                // Keep the tree drawing; the file is the same for every row.
                text.trim_end().to_owned()
            } else {
                format!("{path}:{row}:{col}:{}", text.trim_end())
            };
            Some(location(&label, &abs.display().to_string(), row, col))
        })
        .collect()
}

/// A ripgrep `-C`-style listing of locations with `context` lines around
/// each: `path:line:col:text` for the locations themselves (jumpable in a
/// Kakoune grep buffer) and `path-line-text` for context, with `--` between
/// separate chunks. Paths are shown relative to `cwd`.
pub fn context_view(items: &[String], cwd: &Path, context: usize) -> String {
    // Group by file, keeping first-seen order.
    let mut files: Vec<(&str, Vec<(usize, usize)>)> = Vec::new();
    for item in items {
        let Some((_, (path, line, col))) = parse_location(item) else { continue };
        match files.iter_mut().find(|(p, _)| *p == path) {
            Some((_, locs)) => locs.push((line, col)),
            None => files.push((path, vec![(line, col)])),
        }
    }
    let mut out = String::new();
    for (path, mut locs) in files {
        locs.sort_unstable();
        locs.dedup_by_key(|l| l.0);
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        let shown = Path::new(path).strip_prefix(cwd).map_or_else(|_| path.to_owned(), |p| p.display().to_string());
        let mut last_printed = 0;
        for (i, &(line, col)) in locs.iter().enumerate() {
            let from = line.saturating_sub(context).max(1).max(last_printed + 1);
            let next = locs.get(i + 1).map_or(usize::MAX, |l| l.0);
            let to = (line + context).min(lines.len()).min(next.saturating_sub(1).max(line));
            if !out.is_empty() && from > last_printed + 1 {
                out.push_str("--\n");
            }
            for n in from..=to {
                let text = lines.get(n - 1).copied().unwrap_or_default();
                if n == line {
                    out.push_str(&format!("{shown}:{n}:{col}:{text}\n"));
                } else {
                    out.push_str(&format!("{shown}-{n}-{text}\n"));
                }
            }
            last_printed = to;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_view_shows_surrounding_lines() {
        let dir = tempfile::tempdir().unwrap();
        let src: String = (1..=12).map(|i| format!("line {i}\n")).collect();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, src).unwrap();
        let f = file.display().to_string();
        let items = [location("x", &f, 3, 2), location("y", &f, 4, 1), location("z", &f, 10, 5)];
        let view = context_view(&items, dir.path(), 1);
        assert_eq!(
            view,
            "a.rs-2-line 2\na.rs:3:2:line 3\na.rs:4:1:line 4\na.rs-5-line 5\n--\na.rs-9-line 9\na.rs:10:5:line 10\na.rs-11-line 11\n"
        );
    }

    #[test]
    fn titles() {
        assert_eq!(title("*goto*", "lsp-document-symbol"), "Document Symbols");
        assert_eq!(title("*references*", "lsp-goto"), "References");
        assert_eq!(title("*diagnostics*", "lsp-diagnostics"), "Diagnostics");
        assert_eq!(title("*symbols*", "lsp-goto"), "Workspace Symbols");
        assert_eq!(title("*goto*", "lsp-goto"), "Locations");
    }

    #[test]
    fn document_symbols_resolve_the_current_file() {
        // Captured from kak-lsp with rust-analyzer.
        let content = "src/app.rs\n%:35:11:  const  MAIN_CLIENT\n%:54:6:    impl Layout\n%:55:8:   └── fn  new\n";
        let items = items("lsp-document-symbol", Path::new("/proj"), content);
        assert_eq!(items.len(), 3);
        let (label, (path, line, col)) = parse_location(&items[2]).unwrap();
        assert_eq!((label, path, line, col), ("   └── fn  new", "/proj/src/app.rs", 55, 8));
    }

    #[test]
    fn references_and_diagnostics_keep_their_paths() {
        let content = "src/a.rs:3:5: let x = foo();\nsrc/ui/b.rs:10:1: error: mismatched types\nnot a location\n";
        let items = items("lsp-goto", Path::new("/proj"), content);
        assert_eq!(items.len(), 2);
        let (label, (path, line, col)) = parse_location(&items[1]).unwrap();
        assert_eq!(label, "src/ui/b.rs:10:1: error: mismatched types");
        assert_eq!((path, line, col), ("/proj/src/ui/b.rs", 10, 1));
        assert!(super::items("lsp-diagnostics", Path::new("/proj"), "").is_empty());
    }
}
