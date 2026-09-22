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
