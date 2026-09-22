// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Demo {
    pub features: Vec<crate::features::Feature>,
    pub line_numbers: bool,
    pub title: String,
    pub prefix: crate::prefix::Prefix,
    pub terminal_keys: crate::prefix::TerminalKeys,
    pub header: bool,
    pub cue_list: bool,
    pub cue_width: u16,
    #[serde(rename = "loop")]
    pub loop_cues: bool,
    pub layout: Layout,
    pub panes: Vec<PaneConfig>,
    pub queues: Vec<Queue>,
}

/// Presets remain compatible with existing files; a table defines a split tree.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Layout {
    Preset(LayoutPreset),
    Tree(LayoutNode),
}
impl Default for Layout {
    fn default() -> Self {
        Self::Preset(LayoutPreset::Columns)
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LayoutPreset {
    Columns,
    Rows,
    Grid,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Columns,
    Rows,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged, deny_unknown_fields)]
pub enum LayoutNode {
    Pane {
        pane: String,
        #[serde(default = "unit_weight")]
        weight: u16,
    },
    Split {
        direction: Axis,
        children: Vec<LayoutNode>,
        #[serde(default = "unit_weight")]
        weight: u16,
    },
}
fn unit_weight() -> u16 {
    1
}
impl LayoutNode {
    pub fn weight(&self) -> u16 {
        match self {
            Self::Pane { weight, .. } | Self::Split { weight, .. } => *weight,
        }
    }
    pub fn weight_mut(&mut self) -> &mut u16 {
        match self {
            Self::Pane { weight, .. } | Self::Split { weight, .. } => weight,
        }
    }
    fn validate<'a>(
        &'a self,
        ids: &HashSet<&str>,
        seen: &mut HashSet<&'a str>,
        depth: usize,
    ) -> Result<()> {
        if depth > 16 {
            bail!("Layout nesting must not exceed 16 levels");
        }
        if !(1..=1000).contains(&self.weight()) {
            bail!("Layout weights must be from 1 to 1000");
        }
        match self {
            Self::Pane { pane, .. } => {
                if !ids.contains(pane.as_str()) {
                    bail!("Unknown layout pane: {pane}");
                }
                if !seen.insert(pane) {
                    bail!("Layout pane appears more than once: {pane}");
                }
            }
            Self::Split { children, .. } => {
                if !(2..=16).contains(&children.len()) {
                    bail!("Layout splits need between 2 and 16 children");
                }
                for child in children {
                    child.validate(ids, seen, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}
impl Layout {
    pub fn validate(&self, ids: &HashSet<&str>) -> Result<()> {
        if let Self::Tree(root) = self {
            let mut seen = HashSet::new();
            root.validate(ids, &mut seen, 0)?;
            if &seen != ids {
                bail!("Layout must reference every configured pane exactly once");
            }
        }
        Ok(())
    }
    pub fn add_pane(&mut self, id: &str) {
        if let Self::Tree(root) = self {
            let mut new = LayoutNode::Pane {
                pane: id.into(),
                weight: 1,
            };
            if let LayoutNode::Split {
                direction: Axis::Columns,
                children,
                ..
            } = root
            {
                // Dragging normalizes weights into the hundreds. A default of
                // one would make the new pane nearly invisible in that group.
                *new.weight_mut() = (children
                    .iter()
                    .map(|child| u32::from(child.weight()))
                    .sum::<u32>()
                    / children.len() as u32)
                    .max(1) as u16;
                children.push(new);
            } else {
                let mut previous = root.clone();
                *previous.weight_mut() = 1;
                *root = LayoutNode::Split {
                    direction: Axis::Columns,
                    children: vec![previous, new],
                    weight: 1,
                };
            }
        }
    }
    pub fn cycle(&mut self) {
        *self = Self::Preset(match self {
            Self::Preset(LayoutPreset::Columns) => LayoutPreset::Rows,
            Self::Preset(LayoutPreset::Rows) => LayoutPreset::Grid,
            _ => LayoutPreset::Columns,
        });
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneConfig {
    pub line_numbers: Option<bool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advance_after_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_numbers: Option<bool>,
    pub pane: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyPress>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<Scheme>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPress {
    pub key: String,
    #[serde(default = "one")]
    pub repeat: u16,
}
fn one() -> u16 {
    1
}

impl Command {
    pub fn preview(&self) -> String {
        if self.clear {
            return "Clear pane (native)".into();
        }
        if !self.keys.is_empty() {
            return format!(
                "Send keys: {}",
                self.keys
                    .iter()
                    .map(|key| format!("{} × {}", key.key, key.repeat))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        self.command.clone()
    }
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
        if let Some(show) = command.line_numbers {
            self.line_numbers = Some(show);
        }
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
            line_numbers: None,
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
            features: crate::features::defaults(),
            line_numbers: false,
            title: "nysos demo".into(),
            prefix: crate::prefix::Prefix::default(),
            terminal_keys: crate::prefix::TerminalKeys::Auto,
            header: true,
            cue_list: true,
            cue_width: 26,
            loop_cues: false,
            layout: Layout::default(),
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
    pub fn layout_for(&self, cue: Option<usize>) -> &Layout {
        cue.and_then(|i| self.queues.get(i))
            .and_then(|q| q.layout.as_ref())
            .unwrap_or(&self.layout)
    }
    pub fn layout_for_mut(&mut self, cue: Option<usize>) -> &mut Layout {
        if let Some(i) = cue
            && self.queues.get(i).is_some_and(|q| q.layout.is_some())
        {
            return self.queues[i].layout.as_mut().unwrap();
        }
        &mut self.layout
    }
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
        self.layout.validate(&names)?;
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
        if let Some(layout) = &self.layout {
            layout.validate(panes)?;
        }
        if self.name.trim().is_empty() || self.commands.is_empty() {
            bail!("Each queue needs a name and at least one command");
        }
        if self
            .advance_after_ms
            .is_some_and(|ms| !(1..=86_400_000).contains(&ms))
        {
            bail!("advance_after_ms must be between 1 and 86400000");
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
            let actions = usize::from(!command.command.is_empty())
                + usize::from(!command.keys.is_empty())
                + usize::from(command.clear);
            if actions != 1 {
                bail!("Each pane entry needs exactly one of command, keys, or clear = true");
            }
            let mut presses = 0usize;
            for key in &command.keys {
                crate::input::parse_key(&key.key)?;
                if key.repeat == 0 {
                    bail!("Key repeat must be at least 1");
                }
                presses += usize::from(key.repeat);
            }
            if presses > 4096 {
                bail!("A pane entry allows at most 4096 key presses");
            }
            if !command.command.is_empty()
                && (command.command.trim().is_empty()
                    || command.command.contains(['\0', '\r', '\n']))
            {
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
            advance_after_ms: None,
            layout: None,
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
    fn line_number_gate_defaults_on_but_visibility_defaults_off() {
        assert_eq!(
            Demo::default().features,
            [crate::features::Feature::LineNumbers]
        );
        assert_eq!(
            Demo::parse("").unwrap().features,
            crate::features::defaults()
        );
        assert!(Demo::parse("features = []").unwrap().features.is_empty());
        let builtin = Demo::builtin().unwrap();
        assert!(
            builtin
                .features
                .contains(&crate::features::Feature::LineNumbers)
        );
        assert!(!Demo::default().line_numbers);
        assert!(!Demo::parse("").unwrap().line_numbers);
        assert!(!builtin.line_numbers);
        assert!(
            builtin
                .panes
                .iter()
                .all(|pane| !pane.line_numbers.unwrap_or(builtin.line_numbers))
        );
        assert!(Demo::parse("features = ['unknown']").is_err());
        let demo = Demo::parse("features = ['line-numbers']\nline_numbers = false\n[[panes]]\nid = 'shell'\nline_numbers = true").unwrap();
        assert_eq!(demo.features, [crate::features::Feature::LineNumbers]);
        assert!(!demo.line_numbers);
        assert_eq!(demo.panes[0].line_numbers, Some(true));
    }
    #[test]
    fn action_validation_and_round_trip() {
        let mut demo = test_demo();
        demo.loop_cues = true;
        demo.queues[0].advance_after_ms = Some(250);
        demo.queues[0].commands[0].command.clear();
        demo.queues[0].commands[0].keys = vec![KeyPress {
            key: "Ctrl+C".into(),
            repeat: 4,
        }];
        demo.queues[0].commands[1].command.clear();
        demo.queues[0].commands[1].clear = true;
        let saved = toml::to_string_pretty(&demo).unwrap();
        let loaded = Demo::parse(&saved).unwrap();
        assert!(loaded.loop_cues);
        assert_eq!(loaded.queues[0].commands[0].keys[0].repeat, 4);
        demo.queues[0].commands[0].keys[0].repeat = 0;
        assert!(demo.validate().is_err());
        demo.queues[0].commands[0].keys[0].repeat = 4097;
        assert!(demo.validate().is_err());
        demo.queues[0].commands[0].keys[0].repeat = 1;
        demo.queues[0].commands[0].command = "pwd".into();
        assert!(demo.validate().is_err());
        demo.queues[0].commands[0].command.clear();
        demo.queues[0].advance_after_ms = Some(0);
        assert!(demo.validate().is_err());
    }
    #[test]
    fn nested_layout_round_trip_and_validation() {
        let demo = Demo::parse(include_str!("../examples/nested-layout.toml")).unwrap();
        let saved = toml::to_string_pretty(&demo).unwrap();
        assert_eq!(Demo::parse(&saved).unwrap().layout, demo.layout);
        for layout in [
            r#"{ pane = "missing" }"#,
            r#"{ pane = "presenter" }"#,
            r#"{ direction = "rows", children = [{ pane = "presenter" }, { pane = "presenter" }] }"#,
            r#"{ direction = "rows", children = [] }"#,
            r#"{ direction = "rows", children = [{ pane = "presenter" }] }"#,
            r#"{ direction = "grid", children = [{ pane = "presenter" }, { pane = "observer" }] }"#,
            r#"{ direction = "rows", children = [{ pane = "presenter", weight = 0 }, { pane = "observer" }] }"#,
            r#"{ direction = "rows", weight = 1001, children = [{ pane = "presenter" }, { pane = "observer" }] }"#,
            r#"{ pane = "presenter", direction = "rows", children = [] }"#,
            r#"{ pane = "presenter", typo = 1 }"#,
        ] {
            assert!(
                Demo::parse(&format!("layout = {layout}")).is_err(),
                "accepted {layout}"
            );
        }
        for preset in ["columns", "rows", "grid"] {
            assert!(Demo::parse(&format!("layout = \"{preset}\"")).is_ok());
        }
        let mut demo = Demo {
            panes: vec![PaneConfig::default()],
            layout: Layout::Tree(LayoutNode::Pane {
                pane: "shell".into(),
                weight: 1,
            }),
            ..Default::default()
        };
        demo.validate().unwrap();
        for i in 1..16 {
            let id = format!("extra-{i}");
            demo.layout.add_pane(&id);
            demo.panes.push(PaneConfig {
                id,
                ..Default::default()
            });
            demo.validate().unwrap();
        }
    }
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
        assert_eq!(demo.panes.len(), 3);
        let geometry = crate::layout::panes(
            ratatui::layout::Rect::new(0, 0, 120, 40),
            &demo.layout,
            &demo.panes,
        );
        assert_eq!(
            geometry.panes,
            vec![
                ratatui::layout::Rect::new(0, 0, 60, 40),
                ratatui::layout::Rect::new(60, 0, 60, 20),
                ratatui::layout::Rect::new(60, 20, 60, 20),
            ]
        );
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
