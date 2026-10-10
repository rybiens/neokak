//! Translating terminal key events into Kakoune key syntax.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn named(code: KeyCode) -> Option<&'static str> {
    Some(match code {
        KeyCode::Enter => "ret",
        KeyCode::Tab => "tab",
        KeyCode::Backspace => "backspace",
        KeyCode::Esc => "esc",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Home => "home",
        KeyCode::End => "end",
        KeyCode::PageUp => "pageup",
        KeyCode::PageDown => "pagedown",
        KeyCode::Delete => "del",
        KeyCode::Insert => "ins",
        _ => return None,
    })
}

/// Name a character must use inside `<...>`, if it has one.
fn char_name(c: char) -> Option<&'static str> {
    Some(match c {
        ' ' => "space",
        '<' => "lt",
        '>' => "gt",
        '-' => "minus",
        '+' => "plus",
        ';' => "semicolon",
        '%' => "percent",
        ',' => "comma",
        _ => return None,
    })
}

pub fn to_kak(ev: &KeyEvent) -> Option<String> {
    let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
    let alt = ev.modifiers.contains(KeyModifiers::ALT);
    let shift = ev.modifiers.contains(KeyModifiers::SHIFT);

    let mut prefix = String::new();
    if ctrl {
        prefix.push_str("c-");
    }
    if alt {
        prefix.push_str("a-");
    }

    let body = match ev.code {
        KeyCode::Char(c) => {
            // Shift is already folded into the character itself.
            let c = if shift && c.is_ascii_lowercase() { c.to_ascii_uppercase() } else { c };
            if prefix.is_empty() {
                return Some(match c {
                    '<' => "<lt>".into(),
                    '>' => "<gt>".into(),
                    ' ' => "<space>".into(),
                    _ => c.to_string(),
                });
            }
            char_name(c).map(str::to_owned).unwrap_or_else(|| c.to_string())
        }
        KeyCode::BackTab => {
            return Some(format!("<{prefix}s-tab>"));
        }
        KeyCode::F(n) => format!("f{n}"),
        code => {
            let name = named(code)?;
            if shift {
                prefix.push_str("s-");
            }
            name.to_owned()
        }
    };
    Some(format!("<{prefix}{body}>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> Option<String> {
        to_kak(&KeyEvent::new(code, m))
    }

    #[test]
    fn plain_chars() {
        assert_eq!(k(KeyCode::Char('a'), KeyModifiers::NONE).unwrap(), "a");
        assert_eq!(k(KeyCode::Char('A'), KeyModifiers::SHIFT).unwrap(), "A");
        assert_eq!(k(KeyCode::Char('a'), KeyModifiers::SHIFT).unwrap(), "A");
        assert_eq!(k(KeyCode::Char('<'), KeyModifiers::NONE).unwrap(), "<lt>");
        assert_eq!(k(KeyCode::Char('>'), KeyModifiers::NONE).unwrap(), "<gt>");
        assert_eq!(k(KeyCode::Char(' '), KeyModifiers::NONE).unwrap(), "<space>");
        assert_eq!(k(KeyCode::Char('-'), KeyModifiers::NONE).unwrap(), "-");
        assert_eq!(k(KeyCode::Char('é'), KeyModifiers::NONE).unwrap(), "é");
    }

    #[test]
    fn modifiers() {
        assert_eq!(k(KeyCode::Char('x'), KeyModifiers::CONTROL).unwrap(), "<c-x>");
        assert_eq!(k(KeyCode::Char('x'), KeyModifiers::ALT).unwrap(), "<a-x>");
        assert_eq!(k(KeyCode::Char('X'), KeyModifiers::ALT | KeyModifiers::SHIFT).unwrap(), "<a-X>");
        assert_eq!(k(KeyCode::Char('-'), KeyModifiers::ALT).unwrap(), "<a-minus>");
        assert_eq!(k(KeyCode::Char('<'), KeyModifiers::ALT).unwrap(), "<a-lt>");
        assert_eq!(k(KeyCode::Char(' '), KeyModifiers::CONTROL).unwrap(), "<c-space>");
    }

    #[test]
    fn special_keys() {
        assert_eq!(k(KeyCode::Enter, KeyModifiers::NONE).unwrap(), "<ret>");
        assert_eq!(k(KeyCode::Esc, KeyModifiers::NONE).unwrap(), "<esc>");
        assert_eq!(k(KeyCode::BackTab, KeyModifiers::SHIFT).unwrap(), "<s-tab>");
        assert_eq!(k(KeyCode::Up, KeyModifiers::SHIFT).unwrap(), "<s-up>");
        assert_eq!(k(KeyCode::Left, KeyModifiers::ALT).unwrap(), "<a-left>");
        assert_eq!(k(KeyCode::F(5), KeyModifiers::NONE).unwrap(), "<f5>");
        assert_eq!(k(KeyCode::Null, KeyModifiers::NONE), None);
    }
}
