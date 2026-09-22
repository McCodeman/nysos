use clap::ValueEnum;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

/// Portable prefixes with distinct encodings in ordinary terminals and SSH.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Prefix {
    CtrlA,
    CtrlB,
    #[default]
    CtrlG,
    F12,
}
impl Prefix {
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlA => "Ctrl-A",
            Self::CtrlB => "Ctrl-B",
            Self::CtrlG => "Ctrl-G",
            Self::F12 => "F12",
        }
    }
    pub fn matches(self, key: KeyEvent) -> bool {
        let (code, modifiers) = match self {
            Self::CtrlA => (KeyCode::Char('a'), KeyModifiers::CONTROL),
            Self::CtrlB => (KeyCode::Char('b'), KeyModifiers::CONTROL),
            Self::CtrlG => (KeyCode::Char('g'), KeyModifiers::CONTROL),
            Self::F12 => (KeyCode::F(12), KeyModifiers::NONE),
        };
        key.code == code && key.modifiers == modifiers
    }
}

/// Terminal-specific interpretation of ambiguous Escape + letter sequences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalKeys {
    #[default]
    Auto,
    Standard,
    Ghostty,
}
impl TerminalKeys {
    pub fn detect(program: Option<&str>, term: Option<&str>) -> Self {
        match program.filter(|s| !s.is_empty()) {
            Some(program) if program.eq_ignore_ascii_case("ghostty") => Self::Ghostty,
            // tmux hides the outer terminal identity; TERM may still carry it.
            None | Some("tmux") if term == Some("xterm-ghostty") => Self::Ghostty,
            _ => Self::Standard,
        }
    }
    pub fn resolve(self, detected: Self) -> Self {
        if self == Self::Auto { detected } else { self }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Standard => "standard",
            Self::Ghostty => "ghostty",
        }
    }
}

#[cfg(test)]
mod terminal_tests {
    use super::TerminalKeys::*;
    #[test]
    fn detects_known_terminal_without_guessing_through_multiplexers() {
        for (program, term, expected) in [
            (Some("ghostty"), Some("xterm-256color"), Ghostty),
            (None, Some("xterm-ghostty"), Ghostty),
            (Some("tmux"), Some("tmux-256color"), Standard),
            (Some("iTerm.app"), Some("xterm-ghostty"), Standard),
            (Some("Apple_Terminal"), Some("xterm-256color"), Standard),
            (None, None, Standard),
        ] {
            assert_eq!(super::TerminalKeys::detect(program, term), expected);
        }
        assert_eq!(Auto.resolve(Ghostty), Ghostty);
        assert_eq!(Standard.resolve(Ghostty), Standard);
        assert_eq!(Ghostty.resolve(Standard), Ghostty);
    }
}
