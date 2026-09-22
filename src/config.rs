use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Demo {
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
    pub name: String,
    pub shell: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub scheme: Scheme,
    /// Relative size along the layout's primary axis.
    pub weight: u16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub pane: String,
    pub command: String,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            name: "shell".into(),
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
            header: true,
            cue_list: true,
            cue_width: 26,
            layout: Layout::Columns,
            panes: vec![
                PaneConfig {
                    name: "presenter".into(),
                    ..Default::default()
                },
                PaneConfig {
                    name: "observer".into(),
                    scheme: Scheme::Ember,
                    ..Default::default()
                },
            ],
            queues: vec![Queue {
                name: "Welcome".into(),
                description: "Edit, run or skip these commands; every pane is a live shell.".into(),
                commands: vec![
                    Command {
                        pane: "presenter".into(),
                        command: "printf 'Welcome to nysos!\\n'".into(),
                    },
                    Command {
                        pane: "observer".into(),
                        command: "pwd".into(),
                    },
                ],
            }],
        }
    }
}

impl Demo {
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
        if !(16..=60).contains(&self.cue_width) {
            bail!("cue_width must be between 16 and 60 columns");
        }
        if self.panes.is_empty() || self.panes.len() > 16 {
            bail!("Configure between 1 and 16 panes");
        }
        let mut names = HashSet::new();
        for pane in &self.panes {
            if pane.name.trim().is_empty() || !names.insert(pane.name.as_str()) {
                bail!("Pane names must be nonempty and unique");
            }
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
        for command in &self.commands {
            if !panes.contains(command.pane.as_str()) {
                bail!("Unknown pane: {}", command.pane);
            }
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
mod tests {
    use super::*;
    #[test]
    fn cue_sidebar_defaults_and_width_validation() {
        let demo = Demo::parse("header = false").unwrap();
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
        let mut demo = Demo::default();
        demo.save(&path).unwrap();
        demo.header = false;
        demo.save(&path).unwrap();
        let loaded = Demo::load(&path).unwrap();
        assert!(!loaded.header);
        assert_eq!(loaded.queues[0].commands.len(), 2);
    }
    #[test]
    fn rejects_invalid_targets_and_duplicate_names() {
        let mut demo = Demo::default();
        demo.queues[0].commands[0].pane = "missing".into();
        assert!(demo.validate().is_err());
        demo = Demo::default();
        demo.panes[1].name = demo.panes[0].name.clone();
        assert!(demo.validate().is_err());
    }
    #[test]
    fn rejects_empty_and_multiline_commands() {
        let mut demo = Demo::default();
        demo.panes.clear();
        assert!(demo.validate().is_err());
        demo = Demo::default();
        demo.queues[0].commands[0].command = "echo a\necho b".into();
        assert!(demo.validate().is_err());
    }
}
