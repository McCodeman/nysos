use alacritty_terminal::term::TermMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Ghostty on macOS maps Option-arrows to the same bytes as Alt-B/F.
/// Match the terminal event rather than the OS so this also works through SSH.
pub fn focus_direction(key: KeyEvent, ghostty: bool) -> Option<isize> {
    if key.modifiers != KeyModifiers::ALT {
        return None;
    }
    match key.code {
        KeyCode::Left => Some(-1),
        KeyCode::Right => Some(1),
        KeyCode::Char('b') if ghostty => Some(-1),
        KeyCode::Char('f') if ghostty => Some(1),
        _ => None,
    }
}

pub fn key_bytes(key: KeyEvent, mode: TermMode) -> Vec<u8> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(control);
    let cursor = |suffix: char| {
        if modifier > 1 {
            format!("\x1b[1;{modifier}{suffix}")
        } else if mode.contains(TermMode::APP_CURSOR) {
            format!("\x1bO{suffix}")
        } else {
            format!("\x1b[{suffix}")
        }
    };
    let numbered = |n: u8| {
        if modifier > 1 {
            format!("\x1b[{n};{modifier}~")
        } else {
            format!("\x1b[{n}~")
        }
    };
    let mut bytes = match key.code {
        KeyCode::Char(c) if control && c.is_ascii() => vec![(c.to_ascii_uppercase() as u8) & 0x1f],
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => cursor('A').into_bytes(),
        KeyCode::Down => cursor('B').into_bytes(),
        KeyCode::Right => cursor('C').into_bytes(),
        KeyCode::Left => cursor('D').into_bytes(),
        KeyCode::Home => cursor('H').into_bytes(),
        KeyCode::End => cursor('F').into_bytes(),
        KeyCode::Insert => numbered(2).into_bytes(),
        KeyCode::Delete => numbered(3).into_bytes(),
        KeyCode::PageUp => numbered(5).into_bytes(),
        KeyCode::PageDown => numbered(6).into_bytes(),
        KeyCode::F(n @ 1..=4) => {
            if modifier > 1 {
                format!("\x1b[1;{modifier}{}", (b'P' + n - 1) as char).into_bytes()
            } else {
                vec![0x1b, b'O', b'P' + n - 1]
            }
        }
        KeyCode::F(n @ 5..=12) => {
            numbered([15, 17, 18, 19, 20, 21, 23, 24][n as usize - 5]).into_bytes()
        }
        _ => vec![],
    };
    if alt
        && matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace
        )
    {
        bytes.insert(0, 0x1b);
    }
    bytes
}

pub fn mouse_bytes(event: MouseEvent, x: u16, y: u16, mode: TermMode) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    let button = |b| match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    let (mut code, release) = match event.kind {
        MouseEventKind::Down(b) => (button(b), false),
        MouseEventKind::Up(b) => (button(b), true),
        MouseEventKind::Drag(b)
            if mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) =>
        {
            (button(b) + 32, false)
        }
        MouseEventKind::Moved if mode.contains(TermMode::MOUSE_MOTION) => (35, false),
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        _ => return None,
    };
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        code += 4;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        code += 8;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        code += 16;
    }
    if mode.contains(TermMode::SGR_MOUSE) {
        Some(
            format!(
                "\x1b[<{code};{};{}{}",
                x + 1,
                y + 1,
                if release { 'm' } else { 'M' }
            )
            .into_bytes(),
        )
    } else if x < 223 && y < 223 {
        Some(vec![
            0x1b,
            b'[',
            b'M',
            if release { 35 } else { 32 + code },
            (x + 33) as u8,
            (y + 33) as u8,
        ])
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_accepts_arrow_and_ghostty_word_sequences_only() {
        for (code, direction) in [
            (KeyCode::Left, -1),
            (KeyCode::Right, 1),
            (KeyCode::Char('b'), -1),
            (KeyCode::Char('f'), 1),
        ] {
            assert_eq!(
                focus_direction(KeyEvent::new(code, KeyModifiers::ALT), true),
                Some(direction)
            );
            for modifiers in [
                KeyModifiers::NONE,
                KeyModifiers::CONTROL,
                KeyModifiers::ALT | KeyModifiers::CONTROL,
                KeyModifiers::ALT | KeyModifiers::SHIFT,
            ] {
                assert_eq!(focus_direction(KeyEvent::new(code, modifiers), true), None);
            }
        }
        assert_eq!(
            focus_direction(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT), true),
            None
        );
    }
    #[test]
    fn forwards_shell_and_application_keys() {
        assert_eq!(
            focus_direction(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT), false),
            None
        );
        assert_eq!(
            focus_direction(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT), false),
            Some(1)
        );
        assert_eq!(
            key_bytes(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                TermMode::empty()
            ),
            vec![3]
        );
        assert_eq!(
            key_bytes(
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                TermMode::APP_CURSOR
            ),
            b"\x1bOA"
        );
        assert_eq!(
            key_bytes(
                KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
                TermMode::empty()
            ),
            b"\x1b[1;5D"
        );
    }
    #[test]
    fn sgr_mouse_is_relative_to_pane() {
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 99,
            row: 99,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            mouse_bytes(
                event,
                2,
                3,
                TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE
            )
            .unwrap(),
            b"\x1b[<0;3;4M"
        );
    }
}
