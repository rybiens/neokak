//! Kakoune JSON-RPC UI protocol (`kak -ui json`).
//!
//! Kakoune writes newline-separated notifications with positional params on
//! stdout and reads the same shape on stdin. Parsing is deliberately tolerant:
//! missing or malformed params fall back to defaults so a protocol tweak in a
//! future Kakoune release degrades rendering instead of crashing.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Face {
    pub fg: String,
    pub bg: String,
    pub underline: String,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Atom {
    pub face: Face,
    pub contents: String,
}

pub type Line = Vec<Atom>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Coord {
    pub line: i32,
    pub column: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KakEvent {
    Draw { lines: Vec<Line>, cursor: Coord, default_face: Face, padding_face: Face, widget_columns: i32 },
    DrawStatus { prompt: Line, content: Line, cursor_pos: i32, mode_line: Line, default_face: Face, style: String },
    MenuShow { items: Vec<Line>, anchor: Coord, selected_face: Face, menu_face: Face, style: String },
    MenuSelect(i32),
    MenuHide,
    InfoShow { title: Line, content: Vec<Line>, anchor: Coord, face: Face, style: String },
    InfoHide,
    SetUiOptions(HashMap<String, String>),
    Refresh(bool),
    Unknown(String),
}

/// Positional parameter reader that defaults anything missing or invalid.
struct Params(Vec<Value>);

impl Params {
    fn get<T: for<'de> Deserialize<'de> + Default>(&self, i: usize) -> T {
        self.0.get(i).and_then(|v| T::deserialize(v).ok()).unwrap_or_default()
    }
}

pub fn parse(line: &str) -> anyhow::Result<KakEvent> {
    let mut msg: Value = serde_json::from_str(line)?;
    let method = msg["method"].as_str().unwrap_or_default().to_owned();
    let p = Params(match msg["params"].take() {
        Value::Array(v) => v,
        _ => Vec::new(),
    });
    Ok(match method.as_str() {
        "draw" => KakEvent::Draw {
            lines: p.get(0),
            cursor: p.get(1),
            default_face: p.get(2),
            padding_face: p.get(3),
            widget_columns: p.get(4),
        },
        "draw_status" => KakEvent::DrawStatus {
            prompt: p.get(0),
            content: p.get(1),
            cursor_pos: p.get(2),
            mode_line: p.get(3),
            default_face: p.get(4),
            style: p.get(5),
        },
        "menu_show" => KakEvent::MenuShow {
            items: p.get(0),
            anchor: p.get(1),
            selected_face: p.get(2),
            menu_face: p.get(3),
            style: p.get(4),
        },
        "menu_select" => KakEvent::MenuSelect(p.get(0)),
        "menu_hide" => KakEvent::MenuHide,
        "info_show" => {
            KakEvent::InfoShow { title: p.get(0), content: p.get(1), anchor: p.get(2), face: p.get(3), style: p.get(4) }
        }
        "info_hide" => KakEvent::InfoHide,
        "set_ui_options" => KakEvent::SetUiOptions(p.get(0)),
        "refresh" => KakEvent::Refresh(p.get(0)),
        _ => KakEvent::Unknown(method),
    })
}

pub fn request(method: &str, params: Value) -> String {
    let mut s = json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string();
    s.push('\n');
    s
}

/// Concatenated text of a line, ignoring faces.
pub fn line_text(line: &Line) -> String {
    line.iter().map(|a| a.contents.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from Kakoune 2026.05.21.
    const DRAW: &str = r#"{"jsonrpc":"2.0","method":"draw","params":[[[{"face":{"fg":"black","bg":"white","underline":"default","attributes":[]},"contents":"h"},{"face":{"fg":"default","bg":"default","underline":"default","attributes":[]},"contents":"ello world\n"}]],{"line":0,"column":0},{"fg":"default","bg":"default","underline":"default","attributes":[]},{"fg":"blue","bg":"default","underline":"default","attributes":[]},0]}"#;
    const STATUS: &str = r#"{"jsonrpc":"2.0","method":"draw_status","params":[[{"face":{"fg":"yellow","bg":"default","underline":"default","attributes":[]},"contents":":"}],[{"face":{"fg":"cyan","bg":"default","underline":"default","attributes":[]},"contents":"e"}],1,[{"face":{"fg":"default","bg":"default","underline":"default","attributes":[]},"contents":"t.txt 1:1 "}],{"fg":"cyan","bg":"default","underline":"default","attributes":[]},"command"]}"#;
    const INFO: &str = r#"{"jsonrpc":"2.0","method":"info_show","params":[[{"face":{"fg":"default","bg":"default","underline":"default","attributes":[]},"contents":"user mapping"}],[],{"line":0,"column":0},{"fg":"black","bg":"yellow","underline":"default","attributes":[]},"prompt"]}"#;

    #[test]
    fn parses_draw() {
        let KakEvent::Draw { lines, padding_face, .. } = parse(DRAW).unwrap() else { panic!() };
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "hello world\n");
        assert_eq!(lines[0][0].face.bg, "white");
        assert_eq!(padding_face.fg, "blue");
    }

    #[test]
    fn parses_status() {
        let KakEvent::DrawStatus { prompt, content, cursor_pos, style, .. } = parse(STATUS).unwrap() else { panic!() };
        assert_eq!(line_text(&prompt), ":");
        assert_eq!(line_text(&content), "e");
        assert_eq!(cursor_pos, 1);
        assert_eq!(style, "command");
    }

    #[test]
    fn parses_info() {
        let KakEvent::InfoShow { title, content, style, .. } = parse(INFO).unwrap() else { panic!() };
        assert_eq!(line_text(&title), "user mapping");
        assert!(content.is_empty());
        assert_eq!(style, "prompt");
    }

    #[test]
    fn tolerates_missing_params() {
        let ev = parse(r#"{"jsonrpc":"2.0","method":"draw_status","params":[[]]}"#).unwrap();
        assert!(matches!(ev, KakEvent::DrawStatus { cursor_pos: 0, .. }));
        assert_eq!(parse(r#"{"method":"bogus"}"#).unwrap(), KakEvent::Unknown("bogus".into()));
    }
}
