use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Demo {
    pub title: String,
    pub prefix: crate::prefix::Prefix,
    pub terminal_keys: crate::prefix::TerminalKeys,
    pub header: bool,
    pub cue_list: bool,
    pub cue_width: u16,
    pub layout: Layout,
    pub panes: Vec<PaneConfig>,
    pub queues: Vec<Queue>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Columns,
    Rows,
    Grid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneConfig {
    #[serde(alias = "name")]
    pub id: String,
    pub title: Option<String>,
    pub shell: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub scheme: Scheme,
    /// Relative size along the layout's primary axis.
    pub weight: u16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    #[default]
    Ocean,
    Ember,
    Forest,
    Mono,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub pane: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<Scheme>,
}

fn validate_pane_title(title: Option<&str>) -> Result<()> {
    if title.is_some_and(|title| title.trim().is_empty() || title.chars().any(char::is_control)) {
        bail!("Pane titles must be nonempty and contain no control characters");
    }
    Ok(())
}

impl PaneConfig {
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.id)
    }

    pub fn apply_command_style(&mut self, command: &Command) {
        if let Some(title) = &command.title {
            self.title = Some(title.clone());
        }
        if let Some(scheme) = command.scheme {
            self.scheme = scheme;
        }
    }
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            id: "shell".into(),
            title: None,
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            args: vec![],
            cwd: None,
            scheme: Scheme::Ocean,
            weight: 1,
        }
    }
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            title: "nysos demo".into(),
            prefix: crate::prefix::Prefix::default(),
            terminal_keys: crate::prefix::TerminalKeys::Auto,
            header: true,
            cue_list: true,
            cue_width: 26,
            layout: Layout::Columns,
            panes: vec![
                PaneConfig {
                    id: "presenter".into(),
                    ..Default::default()
                },
                PaneConfig {
                    id: "observer".into(),
                    scheme: Scheme::Ember,
                    ..Default::default()
                },
            ],
            queues: vec![],
        }
    }
}

impl Demo {
    /// Explicitly selected sample, embedded so installation needs no example files.
    pub fn builtin() -> Result<Self> {
        Self::parse(include_str!("../examples/pane-transitions.toml"))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let demo: Self = toml::from_str(text).context("Invalid demo TOML")?;
        demo.validate()?;
        Ok(demo)
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("Reading {}", path.display()))?,
        )
    }

    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() || self.title.chars().any(char::is_control) {
            bail!("Demo title must be nonempty and contain no control characters");
        }
        if !(16..=60).contains(&self.cue_width) {
            bail!("cue_width must be between 16 and 60 columns");
        }
        if self.panes.is_empty() || self.panes.len() > 16 {
            bail!("Configure between 1 and 16 panes");
        }
        let mut names = HashSet::new();
        for pane in &self.panes {
            if pane.id.trim().is_empty()
                || pane.id.chars().any(char::is_control)
                || !names.insert(pane.id.as_str())
            {
                bail!("Pane IDs must be nonempty, unique, and contain no control characters");
            }
            validate_pane_title(pane.title.as_deref())?;
            if pane.shell.trim().is_empty() || pane.weight == 0 || pane.weight > 1000 {
                bail!("Each pane needs a shell and a weight from 1 to 1000");
            }
        }
        for queue in &self.queues {
            queue.validate(&names)?;
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let contents = toml::to_string_pretty(self)?;
        // Write next to the destination, then rename: malformed configs and partial
        // writes cannot destroy the previous demo.
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let temp = parent.join(format!(".nysos-{}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .with_context(|| format!("Creating temporary file for {}", path.display()))?;
        let result = (|| -> Result<()> {
            use std::io::Write;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result.with_context(|| format!("Saving {}", path.display()))
    }
}

impl Queue {
    pub fn validate(&self, panes: &HashSet<&str>) -> Result<()> {
        if self.name.trim().is_empty() || self.commands.is_empty() {
            bail!("Each queue needs a name and at least one command");
        }
        let mut targets = HashSet::new();
        for command in &self.commands {
            if !targets.insert(command.pane.as_str()) {
                bail!(
                    "Cue '{}' has more than one command for pane '{}'; each cue allows at most one command per pane",
                    self.name,
                    command.pane
                );
            }
            if !panes.contains(command.pane.as_str()) {
                bail!("Unknown pane: {}", command.pane);
            }
            validate_pane_title(command.title.as_deref())?;
            if command.command.trim().is_empty() || command.command.contains(['\0', '\r', '\n']) {
                bail!(
                    "Commands must be nonempty single lines; use shell separators for compound commands"
                );
            }
        }
        Ok(())
    }
}

/// An explicit shell keeps real-PTY tests usable inside a Nix build sandbox.
#[cfg(test)]
pub fn test_shell() -> String {
    std::env::var("NYSOS_TEST_SHELL").unwrap_or_else(|_| "/bin/sh".into())
}

/// Avoid user startup files when Nix supplies Bash for PTY tests.
#[cfg(test)]
pub fn test_shell_args() -> Vec<String> {
    if std::path::Path::new(&test_shell())
        .file_name()
        .is_some_and(|name| name == "bash")
    {
        vec!["--noprofile".into(), "--norc".into()]
    } else {
        vec![]
    }
}

#[cfg(test)]
pub fn test_demo() -> Demo {
    Demo {
        queues: vec![Queue {
            name: "Welcome".into(),
            description: "Edit, run or skip these commands; every pane is a live shell.".into(),
            commands: vec![
                Command {
                    pane: "presenter".into(),
                    command: "printf 'Welcome to nysos!\\n'".into(),
                    ..Default::default()
                },
                Command {
                    pane: "observer".into(),
                    command: "pwd".into(),
                    ..Default::default()
                },
            ],
        }],
        ..Demo::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_is_explicit_and_defaults_have_no_cues() {
        assert!(Demo::default().queues.is_empty());
        assert!(Demo::parse("").unwrap().queues.is_empty());
        assert!(
            Demo::parse("[[panes]]\nid = 'custom'")
                .unwrap()
                .queues
                .is_empty()
        );
        let demo = Demo::builtin().unwrap();
        assert_eq!(demo.queues.len(), 6);
        assert_eq!(demo.panes.len(), 2);
        assert_eq!(demo.queues[1].commands.len(), 1);
        assert_eq!(demo.queues[2].commands.len(), 1);
        assert_ne!(
            demo.queues[1].commands[0].pane,
            demo.queues[2].commands[0].pane
        );
        for index in [1, 2] {
            assert!(demo.queues[index].commands[0].title.is_some());
            assert!(demo.queues[index].commands[0].scheme.is_some());
        }
    }

    #[test]
    fn pane_ids_titles_and_legacy_names_round_trip() {
        let demo = Demo::parse(
            r#"
queues = []
[[panes]]
name = "legacy"
[[panes]]
id = "stable"
title = "Display name"
"#,
        )
        .unwrap();
        assert_eq!(demo.panes[0].id, "legacy");
        assert_eq!(demo.panes[0].display_title(), "legacy");
        assert_eq!(demo.panes[1].display_title(), "Display name");
        let text = toml::to_string(&demo).unwrap();
        assert!(!text.contains("name ="));
        assert_eq!(Demo::parse(&text).unwrap().panes[1].id, "stable");
        assert!(Demo::parse("queues = []\n[[panes]]\nid = 'a'\nname = 'b'").is_err());
        for title in ["", " ", "bad\nline", "bad\u{1b}title"] {
            let mut invalid = test_demo();
            invalid.panes[0].title = Some(title.into());
            assert!(invalid.validate().is_err());
            invalid.panes[0].title = None;
            invalid.queues[0].commands[0].title = Some(title.into());
            assert!(invalid.validate().is_err());
        }
        let mut demo = test_demo();
        demo.panes[0].title = Some("Shared title".into());
        demo.panes[1].title = demo.panes[0].title.clone();
        demo.queues[0].commands[0].title = Some("New title".into());
        demo.queues[0].commands[0].scheme = Some(Scheme::Forest);
        let text = toml::to_string(&demo).unwrap();
        let loaded = Demo::parse(&text).unwrap();
        assert_eq!(loaded.queues[0].commands[0].scheme, Some(Scheme::Forest));
        assert!(Demo::parse(&text.replace("forest", "invalid")).is_err());
        demo.queues[0].commands[0].pane = "Shared title".into();
        assert!(demo.validate().is_err()); // Titles are never routing identities.
    }
    #[test]
    fn each_cue_allows_only_one_command_per_pane() {
        let mut demo = test_demo();
        demo.queues.push(demo.queues[0].clone());
        assert!(demo.validate().is_ok()); // Different cues may reuse panes.
        let duplicate = demo.queues[0].commands[0].clone();
        demo.queues[0].commands.push(duplicate);
        let text = toml::to_string(&demo).unwrap();
        let error = Demo::parse(&text).unwrap_err().to_string();
        assert!(error.contains("Welcome"));
        assert!(error.contains("presenter"));
        assert!(error.contains("at most one command per pane"));
        let targets = demo.panes.iter().map(|p| p.id.as_str()).collect();
        assert!(demo.queues[0].validate(&targets).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.toml");
        assert!(demo.save(&path).is_err());
        assert!(!path.exists());
    }
    #[test]
    fn cue_sidebar_defaults_and_width_validation() {
        let demo = Demo::parse("header = false").unwrap();
        assert_eq!(demo.title, "nysos demo");
        assert!(Demo::parse("title = ''").is_err());
        assert!(Demo::parse("title = \"line\\nline\"").is_err());
        assert_eq!(demo.terminal_keys, crate::prefix::TerminalKeys::Auto);
        assert_eq!(
            Demo::parse("terminal_keys = 'standard'")
                .unwrap()
                .terminal_keys,
            crate::prefix::TerminalKeys::Standard
        );
        assert!(Demo::parse("terminal_keys = 'unknown'").is_err());
        assert_eq!(demo.prefix, crate::prefix::Prefix::CtrlG);
        assert_eq!(
            Demo::parse("prefix = 'ctrl-b'").unwrap().prefix,
            crate::prefix::Prefix::CtrlB
        );
        assert!(Demo::parse("prefix = 'ctrl-space'").is_err());
        assert!(demo.cue_list);
        assert_eq!(demo.cue_width, 26);
        let demo = Demo::parse("cue_list = false\ncue_width = 40").unwrap();
        assert!(!demo.cue_list);
        assert_eq!(demo.cue_width, 40);
        assert!(Demo::parse("cue_width = 15").is_err());
        assert!(Demo::parse("cue_width = 61").is_err());
    }
    #[test]
    fn round_trip_and_atomic_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.toml");
        let mut demo = test_demo();
        demo.save(&path).unwrap();
        demo.header = false;
        demo.save(&path).unwrap();
        let loaded = Demo::load(&path).unwrap();
        assert!(!loaded.header);
        assert_eq!(loaded.queues[0].commands.len(), 2);
    }
    #[test]
    fn rejects_invalid_targets_and_duplicate_names() {
        let mut demo = test_demo();
        demo.queues[0].commands[0].pane = "missing".into();
        assert!(demo.validate().is_err());
        demo = test_demo();
        demo.panes[1].id = demo.panes[0].id.clone();
        assert!(demo.validate().is_err());
    }
    #[test]
    fn rejects_empty_and_multiline_commands() {
        let mut demo = test_demo();
        demo.panes.clear();
        assert!(demo.validate().is_err());
        demo = test_demo();
        demo.queues[0].commands[0].command = "echo a\necho b".into();
        assert!(demo.validate().is_err());
    }
}
