//! Turning kak-lsp's result lists into picker items.
//!
//! kak-lsp shows symbols, references, diagnostics and similar lists through
//! its `lsp-show-goto-buffer` command (buffer name, filetype, project root,
//! content). `rc/neokak.kak` overrides that command to send the results here
//! instead. Each content line is `path:line:col:text`, with paths relative to
//! the project root. In document-symbol lists the first line names the file
//! and `%` stands for it.

use std::path::Path;

use crate::ui::picker::{location, parse_grep};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::picker::parse_location;

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
