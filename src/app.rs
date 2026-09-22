use crate::{
    config::{Demo, Layout, PaneConfig, Queue},
    cues::CueList,
    input, layout,
    pane::{Pane, palette},
};
use alacritty_terminal::grid::Scroll;
use anyhow::{Result, bail};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout as UiLayout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::{collections::HashMap, path::PathBuf, process::Command};
use tui_textarea::TextArea;

#[derive(Clone, Copy, PartialEq)]
enum EditorKind {
    Demo,
    Queue(usize),
    LoadPath,
    SavePath,
}
struct Editor {
    kind: EditorKind,
    text: TextArea<'static>,
    error: String,
}
impl Editor {
    fn new(kind: EditorKind, text: String) -> Self {
        let mut text = TextArea::from(text.lines());
        text.set_line_number_style(Style::default().fg(Color::DarkGray));
        text.set_cursor_line_style(Style::default().bg(Color::Rgb(35, 45, 60)));
        Self {
            kind,
            text,
            error: String::new(),
        }
    }
    fn content(&self) -> String {
        self.text.lines().join("\n")
    }
}

pub struct App {
    pub demo: Demo,
    pub panes: Vec<Pane>,
    pub active: usize,
    queue: usize,
    command: usize,
    pub path: PathBuf,
    prefix: bool,
    detected_terminal: crate::prefix::TerminalKeys,
    editor: Option<Editor>,
    pending_demo: Option<Demo>,
    help: bool,
    preview: bool,
    preview_scroll: u16,
    pub quit: bool,
    pub debug_keys: bool,
    last_key: String,
    status: String,
    rects: Vec<Rect>,
    drag: Option<usize>,
    cues: CueList,
}
impl App {
    pub fn new(demo: Demo, path: PathBuf) -> Result<Self> {
        demo.validate()?;
        let panes = demo
            .panes
            .iter()
            .cloned()
            .map(Pane::spawn)
            .collect::<Result<_>>()?;
        let detected_terminal = crate::prefix::TerminalKeys::detect(
            std::env::var("TERM_PROGRAM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
        );
        let terminal_label = demo.terminal_keys.resolve(detected_terminal).label();
        Ok(Self {
            demo,
            panes,
            active: 0,
            queue: 0,
            command: 0,
            path,
            prefix: false,
            detected_terminal,
            editor: None,
            pending_demo: None,
            help: false,
            preview: false,
            preview_scroll: 0,
            quit: false,
            debug_keys: false,
            last_key: "Waiting for a key".into(),
            status: format!(
                "Keys: {terminal_label} · Prefix then ? for help · n runs the next command"
            ),
            rects: vec![],
            drag: None,
            cues: CueList::default(),
        })
    }
    pub fn tick(&mut self) -> Result<()> {
        for pane in &mut self.panes {
            pane.pump()?;
        }
        Ok(())
    }
    pub fn resize(&mut self, area: Rect) -> Result<()> {
        let header = if self.demo.header { 4 } else { 0 };
        let chunks = UiLayout::vertical([
            Constraint::Length(header),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);
        // Keep some shell space even in a very small terminal.
        let width = if self.demo.cue_list {
            self.demo.cue_width.min(chunks[1].width / 2)
        } else {
            0
        };
        let horizontal =
            UiLayout::horizontal([Constraint::Length(width), Constraint::Min(0)]).split(chunks[1]);
        self.cues.resize(horizontal[0], self.demo.queues.len());
        self.rects = layout::panes(horizontal[1], self.demo.layout, &self.demo.panes);
        for (pane, rect) in self.panes.iter_mut().zip(&self.rects) {
            pane.resize(inner(*rect))?;
        }
        Ok(())
    }
    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let header = if self.demo.header { 4 } else { 0 };
        let chunks = UiLayout::vertical([
            Constraint::Length(header),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);
        if self.demo.header {
            let queue_index = if self.cues.focused {
                self.cues.selected
            } else {
                self.queue
            };
            let command_index = if queue_index == self.queue {
                self.command
            } else {
                0
            };
            let text = if let Some(queue) = self.demo.queues.get(queue_index) {
                format!(
                    "{}/{} · {} · command {}/{}\n{}",
                    queue_index + 1,
                    self.demo.queues.len(),
                    queue.name,
                    command_index + 1,
                    queue.commands.len(),
                    queue.description
                )
            } else {
                "Demo complete · prefix o to edit or load another demo".into()
            };
            frame.render_widget(
                Paragraph::new(format!("{}\n{text}", self.demo.title))
                    .style(Style::default().fg(Color::White).bg(Color::Rgb(28, 35, 48))),
                chunks[0],
            );
        }
        if self.demo.cue_list {
            self.cues
                .draw(frame, &self.demo.queues, self.queue, self.command);
        }
        for (i, (pane, &area)) in self.panes.iter().zip(&self.rects).enumerate() {
            let (_, bg, accent) = palette(pane.config.scheme);
            let title = format!(
                " {} · {}{} ",
                i + 1,
                pane.config.display_title(),
                if pane.exited { " [exited]" } else { "" }
            );
            frame.render_widget(
                Block::bordered()
                    .title(title)
                    .border_style(
                        Style::default().fg(if i == self.active && !self.cues.focused {
                            accent
                        } else {
                            Color::DarkGray
                        }),
                    )
                    .style(Style::default().bg(bg)),
                area,
            );
            pane.render(inner(area), frame.buffer_mut());
        }
        let status = if self.prefix {
            "PREFIX: n run · s skip · e edit cue · o edit demo/add cues · a add · Tab focus · c cues · 0 focus cues · l layout · h header · x restart · q quit · ? help"
        } else {
            &self.status
        };
        let hints = if self.preview {
            "PREVIEW: ↑↓ scroll · Esc close · no commands executed"
        } else if self.cues.focused {
            "CUES: ↑↓ select · Enter run · p preview · t type · e edit · o demo/add cues"
        } else {
            "Ctrl-G: controls   Alt-←/→: focus   Click: focus   Drag border: resize   Alt-click: URL"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(status),
                Line::from(hints.replace("Ctrl-G", self.demo.prefix.label())),
            ])
            .style(Style::default().fg(Color::Gray)),
            chunks[2],
        );
        if self.preview {
            for (pane, rect) in self.panes.iter().zip(&self.rects) {
                let body = inner(*rect);
                let width = body.width.min(68);
                let height = body.height.min(8);
                let area = Rect::new(
                    body.x + (body.width - width) / 2,
                    body.y + (body.height - height) / 2,
                    width,
                    height,
                );
                let (next, count) = self.pane_preview(&pane.config.id);
                let title = format!(
                    " Preview · {} · {} pending ",
                    pane.config.display_title(),
                    count
                );
                let text = next.unwrap_or("No pending command for this cue");
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(text)
                        .block(Block::bordered().title(title))
                        .style(Style::default().fg(Color::White).bg(Color::Rgb(28, 35, 48)))
                        .wrap(Wrap { trim: false })
                        .scroll((
                            if next.is_some() {
                                self.preview_scroll
                            } else {
                                0
                            },
                            0,
                        )),
                    area,
                );
            }
        } else if let Some(editor) = &self.editor {
            let area = popup(area, 92, 88);
            frame.render_widget(Clear, area);
            let title = match editor.kind {
                EditorKind::Demo => {
                    " Full demo TOML · Ctrl-L load · Ctrl-S save as · Ctrl-G apply · Esc cancel "
                }
                EditorKind::Queue(_) => " Single cue TOML · Ctrl-G apply · Esc cancel ",
                EditorKind::LoadPath => " Load path · Enter load · Esc cancel ",
                EditorKind::SavePath => " Save path · Enter save & apply · Esc cancel ",
            };
            frame.render_widget(
                Block::bordered()
                    .title(title)
                    .style(Style::default().bg(Color::Rgb(20, 26, 36))),
                area,
            );
            let parts =
                UiLayout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(inner(area));
            frame.render_widget(&editor.text, parts[0]);
            frame.render_widget(
                Paragraph::new(if !editor.error.is_empty() {
                    editor.error.as_str()
                } else if editor.kind == EditorKind::Demo {
                    "Use Control, not Cmd; no prefix. F2 load · F3 save · F4 apply. Edit title at top; add/reorder [[queues]] below."
                } else if matches!(editor.kind, EditorKind::Queue(_)) {
                    "Editing one cue. Esc then prefix-o opens the full demo to add cues or edit its title."
                } else {
                    "Enter a file path; Enter confirms and Esc cancels."
                })
                    .style(Style::default().fg(Color::LightRed))
                    .wrap(Wrap { trim: false }),
                parts[1],
            );
        } else if self.help {
            let area = popup(area, 88, 85);
            frame.render_widget(Clear, area);
            let help = HELP.replace(
                "Press Ctrl-G",
                &format!("Press {}", self.demo.prefix.label()),
            );
            let profile = self
                .demo
                .terminal_keys
                .resolve(self.detected_terminal)
                .label();
            let title = format!(" nysos controls · keys: {profile} · Esc closes ");
            frame.render_widget(
                Paragraph::new(help)
                    .block(Block::bordered().title(title))
                    .style(Style::default().bg(Color::Rgb(20, 26, 36)))
                    .wrap(Wrap { trim: false }),
                area,
            );
        } else if !self.cues.focused
            && let Some(rect) = self.rects.get(self.active)
            && let Some(pos) = self.panes[self.active].cursor(inner(*rect))
        {
            frame.set_cursor_position(pos);
        }
        if self.debug_keys && area.height > 0 {
            let mode = match self.editor.as_ref().map(|editor| editor.kind) {
                Some(EditorKind::Demo) => "full demo",
                Some(EditorKind::Queue(_)) => "single cue",
                Some(EditorKind::LoadPath) => "load path",
                Some(EditorKind::SavePath) => "save path",
                None if self.help => "help",
                None if self.preview => "cue preview",
                None if self.cues.focused => "cue list",
                None => "shell",
            };
            let row = Rect::new(area.x, area.bottom() - 1, area.width, 1);
            frame.render_widget(Clear, row);
            frame.render_widget(
                Paragraph::new(format!("Input: {} | Mode: {mode}", self.last_key))
                    .style(Style::default().fg(Color::Yellow).bg(Color::Black)),
                row,
            );
        }
    }
    pub fn event(&mut self, event: Event) {
        if self.debug_keys {
            match &event {
                Event::Key(key) => {
                    self.last_key = format!("{:?} {:?} {:?}", key.code, key.modifiers, key.kind);
                }
                Event::Paste(_) => self.last_key = "Paste (not a shortcut)".into(),
                _ => {}
            }
        }
        let result = self.handle(event);
        if let Err(error) = result {
            if let Some(editor) = &mut self.editor {
                editor.error = if let Some(parse) = error.downcast_ref::<toml::de::Error>() {
                    format!("Invalid TOML: {}\n\n{error:#}", parse.message())
                } else {
                    format!("{error:#}")
                };
            } else {
                self.status = format!("Error: {error:#}");
            }
        }
    }
    fn handle(&mut self, event: Event) -> Result<()> {
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Release
        {
            return Ok(());
        }
        if self.editor.is_some() {
            return self.editor_event(event);
        }
        if self.preview {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Esc => self.preview = false,
                    KeyCode::Up => self.preview_scroll = self.preview_scroll.saturating_sub(1),
                    KeyCode::Down => self.preview_scroll = self.preview_scroll.saturating_add(1),
                    _ => {}
                }
            }
            return Ok(());
        }
        if self.help {
            if matches!(
                event,
                Event::Key(KeyEvent {
                    code: KeyCode::Esc,
                    ..
                })
            ) {
                self.help = false;
            }
            return Ok(());
        }
        match event {
            Event::Key(key) => {
                if self.prefix {
                    self.prefix = false;
                    if self.demo.prefix.matches(key) {
                        if !self.cues.focused {
                            let bytes = input::key_bytes(key, *self.panes[self.active].term.mode());
                            self.panes[self.active].write(&bytes)?;
                        }
                        return Ok(());
                    }
                    match key.code {
                        KeyCode::Char('q') => self.quit = true,
                        KeyCode::Char('n') | KeyCode::Enter => self.step(false)?,
                        KeyCode::Char('s') => self.step(true)?,
                        KeyCode::Char('e') => self.edit_queue(if self.cues.focused {
                            self.cues.selected
                        } else {
                            self.queue
                        })?,
                        KeyCode::Char('c') => self.toggle_cues(),
                        KeyCode::Char('0') => {
                            self.demo.cue_list = true;
                            self.cues.focused = true;
                        }
                        KeyCode::Char('o') => self.edit_demo()?,
                        KeyCode::Char('a') => self.add_pane()?,
                        KeyCode::Tab | KeyCode::Right => self.rotate(1),
                        KeyCode::BackTab | KeyCode::Left => self.rotate(-1),
                        KeyCode::Char('h') => self.demo.header = !self.demo.header,
                        KeyCode::Char('l') => {
                            self.demo.layout = match self.demo.layout {
                                Layout::Columns => Layout::Rows,
                                Layout::Rows => Layout::Grid,
                                Layout::Grid => Layout::Columns,
                            }
                        }
                        KeyCode::Char('x') if !self.cues.focused => {
                            self.panes[self.active] =
                                Pane::spawn(self.panes[self.active].config.clone())?
                        }
                        KeyCode::Char('?') => self.help = true,
                        KeyCode::Char(c @ '1'..='9') => {
                            let i = (c as u8 - b'1') as usize;
                            if i < self.panes.len() {
                                self.active = i;
                                self.cues.focused = false;
                            }
                        }
                        _ => {}
                    }
                } else if self.demo.prefix.matches(key) {
                    self.prefix = true;
                } else if let Some(direction) = input::focus_direction(
                    key,
                    self.demo.terminal_keys.resolve(self.detected_terminal)
                        == crate::prefix::TerminalKeys::Ghostty,
                ) {
                    self.rotate(direction);
                } else if self.cues.focused {
                    self.cue_key(key)?;
                } else {
                    let bytes = input::key_bytes(key, *self.panes[self.active].term.mode());
                    self.panes[self.active].write(&bytes)?;
                }
            }
            Event::Paste(text) if !self.cues.focused => self.panes[self.active].paste(&text)?,
            Event::Mouse(mouse) => self.mouse(mouse)?,
            _ => {}
        }
        Ok(())
    }
    fn rotate(&mut self, direction: isize) {
        let extra = usize::from(self.demo.cue_list);
        let position = if self.cues.focused && self.demo.cue_list {
            0
        } else {
            self.active + extra
        };
        let next = (position as isize + direction).rem_euclid((self.panes.len() + extra) as isize)
            as usize;
        self.cues.focused = extra == 1 && next == 0;
        if !self.cues.focused {
            self.active = next - extra;
        }
    }
    fn toggle_cues(&mut self) {
        self.demo.cue_list = !self.demo.cue_list;
        if !self.demo.cue_list {
            self.cues.focused = false;
        }
    }
    fn edit_demo(&mut self) -> Result<()> {
        self.editor = Some(Editor::new(
            EditorKind::Demo,
            toml::to_string_pretty(&self.demo)?,
        ));
        Ok(())
    }
    fn pane_preview(&self, pane: &str) -> (Option<&str>, usize) {
        let Some(cue) = self.demo.queues.get(self.cues.selected) else {
            return (None, 0);
        };
        let start = if self.cues.selected == self.queue {
            self.command
        } else {
            0
        };
        let mut commands = cue
            .commands
            .iter()
            .skip(start)
            .filter(|command| command.pane == pane);
        let next = commands.next().map(|command| command.command.as_str());
        let count = usize::from(next.is_some()) + commands.count();
        (next, count)
    }
    fn type_selected(&mut self) -> Result<()> {
        let commands: Vec<_> = self
            .panes
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                self.pane_preview(&pane.config.id)
                    .0
                    .map(|command| (index, command.to_owned()))
            })
            .collect();
        let Some((first, _)) = commands.first() else {
            self.status = "No pending commands to type for this cue".into();
            return Ok(());
        };
        // Preflight every destination before modifying any pane's input buffer.
        for (index, command) in &commands {
            if command.chars().any(char::is_control) {
                bail!("Typing requires commands without control characters (including tabs)");
            }
            let pane = &mut self.panes[*index];
            pane.pump()?;
            if pane.exited {
                bail!(
                    "Pane '{}' has exited; restart it before typing this cue",
                    pane.config.id
                );
            }
        }
        for (index, command) in &commands {
            self.panes[*index].paste(command)?;
            let action = self.demo.queues[self.cues.selected]
                .commands
                .iter()
                .find(|action| action.pane == self.panes[*index].config.id)
                .expect("validated target");
            self.panes[*index].config.apply_command_style(action);
        }
        self.active = *first;
        self.cues.focused = false;
        self.status = "Typed without Enter · edit in each shell · cue progress unchanged".into();
        Ok(())
    }
    fn edit_queue(&mut self, index: usize) -> Result<()> {
        if let Some(queue) = self.demo.queues.get(index) {
            self.editor = Some(Editor::new(
                EditorKind::Queue(index),
                toml::to_string_pretty(queue)?,
            ));
        }
        Ok(())
    }
    fn cue_key(&mut self, key: KeyEvent) -> Result<()> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return Ok(());
        }
        let count = self.demo.queues.len();
        match key.code {
            KeyCode::Up => self.cues.move_by(-1, count),
            KeyCode::Down => self.cues.move_by(1, count),
            KeyCode::Home => self.cues.select(0, count),
            KeyCode::End => self.cues.select(count.saturating_sub(1), count),
            KeyCode::PageUp => self.cues.move_by(
                -(self.cues.area.height.saturating_sub(2).max(1) as isize),
                count,
            ),
            KeyCode::PageDown => self.cues.move_by(
                self.cues.area.height.saturating_sub(2).max(1) as isize,
                count,
            ),
            KeyCode::Enter => self.execute_selected()?,
            KeyCode::Char('t') => self.type_selected()?,
            KeyCode::Char('p') if count > 0 => {
                self.preview = true;
                self.preview_scroll = 0;
            }
            KeyCode::Char('e') => self.edit_queue(self.cues.selected)?,
            KeyCode::Char('o') => self.edit_demo()?,
            KeyCode::Tab | KeyCode::Right => self.rotate(1),
            KeyCode::BackTab | KeyCode::Left => self.rotate(-1),
            KeyCode::Esc => self.cues.focused = false,
            _ => {}
        }
        Ok(())
    }
    fn execute_selected(&mut self) -> Result<()> {
        let index = self.cues.selected;
        let Some(cue) = self.demo.queues.get(index) else {
            self.status = format!(
                "No cues configured · {} o to add them",
                self.demo.prefix.label()
            );
            return Ok(());
        };
        let start = if index == self.queue { self.command } else { 0 };
        // Reject known-dead targets before sending any part of a multi-pane cue.
        for command in &cue.commands[start..] {
            let pane = self
                .panes
                .iter_mut()
                .find(|p| p.config.id == command.pane)
                .expect("validated target");
            pane.pump()?;
            if pane.exited {
                bail!(
                    "Pane '{}' has exited; focus it and use {} x before running this cue",
                    command.pane,
                    self.demo.prefix.label()
                );
            }
        }
        self.queue = index;
        self.command = start;
        while self.queue == index {
            self.step(false)?;
        }
        self.cues.select(self.queue, self.demo.queues.len());
        Ok(())
    }
    fn step(&mut self, skip: bool) -> Result<()> {
        let Some(queue) = self.demo.queues.get(self.queue) else {
            self.status = "Demo complete".into();
            return Ok(());
        };
        let command = &queue.commands[self.command];
        if !skip {
            let pane = self
                .panes
                .iter_mut()
                .find(|p| p.config.id == command.pane)
                .expect("validated target");
            pane.write(format!("{}\r", command.command).as_bytes())?;
            pane.config.apply_command_style(command);
        }
        self.status.clear();
        self.command += 1;
        if self.command == queue.commands.len() {
            self.queue += 1;
            self.command = 0;
        }
        if !self.cues.focused {
            self.cues.select(self.queue, self.demo.queues.len());
        }
        Ok(())
    }
    fn add_pane(&mut self) -> Result<()> {
        if self.panes.len() >= 16 {
            bail!("Maximum of 16 panes reached");
        }
        let name = (1..)
            .map(|n| format!("adhoc-{n}"))
            .find(|n| !self.demo.panes.iter().any(|p| p.id == *n))
            .unwrap();
        let config = PaneConfig {
            id: name,
            ..Default::default()
        };
        let pane = Pane::spawn(config.clone())?;
        self.demo.panes.push(config);
        self.panes.push(pane);
        self.active = self.panes.len() - 1;
        self.cues.focused = false;
        self.status = "Added a live pane · prefix o to rename or configure it".into();
        Ok(())
    }
    fn apply(&mut self, demo: Demo) -> Result<()> {
        demo.validate()?;
        // Prepare new sessions first; a failed shell spawn leaves the running demo intact.
        let mut replacements = HashMap::new();
        for config in &demo.panes {
            let reuse = self.panes.iter().any(|p| {
                p.config.id == config.id
                    && p.config.shell == config.shell
                    && p.config.args == config.args
                    && p.config.cwd == config.cwd
            });
            if !reuse {
                replacements.insert(config.id.clone(), Pane::spawn(config.clone())?);
            }
        }
        let mut old: HashMap<_, _> = self
            .panes
            .drain(..)
            .map(|p| (p.config.id.clone(), p))
            .collect();
        for config in &demo.panes {
            let mut pane = replacements
                .remove(&config.id)
                .or_else(|| old.remove(&config.id))
                .expect("prepared pane");
            pane.config = config.clone();
            self.panes.push(pane);
        }
        self.demo = demo;
        self.active = self.active.min(self.panes.len() - 1);
        self.queue = 0;
        self.command = 0;
        self.cues.select(0, self.demo.queues.len());
        self.cues.focused &= self.demo.cue_list;
        self.status =
            "Configuration applied · queue rewound · matching shell sessions preserved".into();
        Ok(())
    }
    fn editor_event(&mut self, event: Event) -> Result<()> {
        let Some(editor) = &self.editor else {
            return Ok(());
        };
        if let Event::Key(key) = event {
            if key.code == KeyCode::Esc {
                self.editor = None;
                self.pending_demo = None;
                return Ok(());
            }
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            let kind = editor.kind;
            if kind == EditorKind::Demo
                && ((ctrl && key.code == KeyCode::Char('l')) || key.code == KeyCode::F(2))
            {
                self.editor = Some(Editor::new(
                    EditorKind::LoadPath,
                    self.path.to_string_lossy().into_owned(),
                ));
                return Ok(());
            }
            if kind == EditorKind::Demo
                && ((ctrl && key.code == KeyCode::Char('s')) || key.code == KeyCode::F(3))
            {
                self.pending_demo = Some(Demo::parse(&editor.content())?);
                self.editor = Some(Editor::new(
                    EditorKind::SavePath,
                    self.path.to_string_lossy().into_owned(),
                ));
                return Ok(());
            }
            if matches!(kind, EditorKind::LoadPath | EditorKind::SavePath)
                && key.code == KeyCode::Enter
            {
                let path = PathBuf::from(editor.content().trim());
                if path.as_os_str().is_empty() {
                    bail!("Enter a file path");
                }
                if kind == EditorKind::LoadPath {
                    let demo = Demo::load(&path)?;
                    self.path = path;
                    self.editor = Some(Editor::new(
                        EditorKind::Demo,
                        toml::to_string_pretty(&demo)?,
                    ));
                } else {
                    let demo = self.pending_demo.as_ref().expect("pending save").clone();
                    demo.save(&path)?;
                    self.apply(demo)?;
                    self.path = path;
                    self.editor = None;
                    self.pending_demo = None;
                }
                return Ok(());
            }
            if (ctrl && key.code == KeyCode::Char('g')) || key.code == KeyCode::F(4) {
                match kind {
                    EditorKind::Demo => self.apply(Demo::parse(&editor.content())?)?,
                    EditorKind::Queue(index) => {
                        let queue: Queue = toml::from_str(&editor.content())?;
                        queue.validate(&self.demo.panes.iter().map(|p| p.id.as_str()).collect())?;
                        self.demo.queues[index] = queue;
                        if index == self.queue {
                            self.command = 0;
                        }
                        self.status =
                            "Cue edited · no commands executed; current cue rewound if edited"
                                .into();
                    }
                    _ => return Ok(()),
                }
                self.editor = None;
                return Ok(());
            }
        }
        match event {
            Event::Paste(text) => {
                self.editor.as_mut().unwrap().text.insert_str(text);
            }
            Event::Key(key) => {
                self.editor.as_mut().unwrap().text.input(key);
            }
            _ => {}
        }
        Ok(())
    }
    fn mouse(&mut self, event: MouseEvent) -> Result<()> {
        let (x, y) = (event.column, event.row);
        if matches!(event.kind, MouseEventKind::Up(_)) {
            self.drag = None;
        }
        if matches!(event.kind, MouseEventKind::Drag(MouseButton::Left))
            && let Some(index) = self.drag
        {
            layout::resize_pair(
                &mut self.demo.panes,
                &self.rects,
                self.demo.layout,
                index,
                x,
                y,
            );
            return Ok(());
        }
        if self.demo.cue_list && self.cues.area.contains((x, y).into()) {
            match event.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.cues.focused = true;
                    if let Some(index) = self.cues.row_at(x, y, self.demo.queues.len()) {
                        self.cues.select(index, self.demo.queues.len());
                    }
                }
                MouseEventKind::ScrollUp => self.cues.move_by(-1, self.demo.queues.len()),
                MouseEventKind::ScrollDown => self.cues.move_by(1, self.demo.queues.len()),
                _ => {}
            }
            return Ok(());
        }
        let Some(index) = self.rects.iter().position(|r| r.contains((x, y).into())) else {
            return Ok(());
        };
        let rect = self.rects[index];
        let body = inner(rect);
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            self.active = index;
            self.cues.focused = false;
            if !body.contains((x, y).into()) {
                let vertical = self.demo.layout == Layout::Rows;
                if (vertical && y == rect.bottom().saturating_sub(1))
                    || (!vertical && x == rect.right().saturating_sub(1))
                {
                    self.drag = Some(index);
                } else if index > 0 && ((vertical && y == rect.y) || (!vertical && x == rect.x)) {
                    self.drag = Some(index - 1);
                }
                return Ok(());
            }
            if event
                .modifiers
                .intersects(KeyModifiers::SUPER | KeyModifiers::ALT)
                || crate::platform::command_pressed()
            {
                if let Some(url) = self.panes[index].url_at(x - body.x, y - body.y) {
                    open_url(&url)?;
                    self.status = format!("Opened {url}");
                }
                return Ok(());
            }
        }
        if !body.contains((x, y).into()) {
            return Ok(());
        }
        if !event.modifiers.contains(KeyModifiers::SHIFT)
            && let Some(bytes) = input::mouse_bytes(
                event,
                x - body.x,
                y - body.y,
                *self.panes[index].term.mode(),
            )
        {
            self.panes[index].write(&bytes)?;
        } else {
            match event.kind {
                MouseEventKind::ScrollUp => self.panes[index].term.scroll_display(Scroll::Delta(3)),
                MouseEventKind::ScrollDown => {
                    self.panes[index].term.scroll_display(Scroll::Delta(-3))
                }
                _ => {}
            }
        }
        Ok(())
    }
}
fn inner(rect: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(rect)
}
fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let w = area.width.saturating_mul(width) / 100;
    let h = area.height.saturating_mul(height) / 100;
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
fn open_url(url: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
const HELP: &str = "Every pane is a live PTY shell. Type normally; Ctrl-C reaches the shell.\n\nPress Ctrl-G, release, then:\n  n / Enter    Send the next command and advance\n  s            Skip the next command\n  e            Edit current queue item before running it\n  o            Edit full demo (title, add/reorder cues, panes, layout)\n  a            Add and focus an ad hoc shell\n  Tab / →      Focus next pane; Shift-Tab / ← goes back\n  0            Show and focus the cue list\n  c            Show/hide the cue list\n  1–9          Focus shell pane by number\n  l / h        Cycle layout / toggle header\n  x            Restart focused shell (ends its current session)\n  q            Quit and close all shells\n\nIn Cues: ↑/↓ browse, Enter sends the selected cue, e edits it; o edits the entire demo.\np previews the next command per pane; Esc closes, arrows scroll.\nt types those commands without Enter, then focuses the first target shell.\nTab/Esc returns to a shell. Enter resumes a partially sent current cue.\nCommands are dispatched in order without waiting for completion.\n\nAlt-Left/Right rotates focus, including Cues. Ghostty mappings also accept Alt-B/F. Click a pane to focus.\nDrag a shared border to resize. Grid rows are equal height.\nScroll wheel uses scrollback; Shift-wheel overrides application mouse mode.\nAlt-click opens HTTP(S) links. Cmd-click works locally on macOS outside tmux when the host forwards the click.\nOver SSH, URL openers run on the remote host.\n\nEditor: Ctrl-L load, Ctrl-S save as, Ctrl-G apply, Esc cancel.\nLoading previews the file; applying resets queue progress. Changing a pane\nid/shell/args/cwd creates a new session. Removed sessions are closed.\nCommands are sent to the pane's current foreground program: wait for its\nprompt before running the next command. No automatic completion detection.";

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::config::Command as DemoCommand;
    use ratatui::{Terminal, backend::TestBackend};
    fn app() -> App {
        let mut demo = Demo::default();
        for pane in &mut demo.panes {
            pane.shell = crate::config::test_shell();
            pane.args = crate::config::test_shell_args();
        }
        App::new(demo, "demo.toml".into()).unwrap()
    }
    fn key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        app.event(Event::Key(KeyEvent::new(code, modifiers)));
    }
    #[test]
    fn cue_styles_apply_only_on_dispatch_and_preserve_identity() {
        use crate::config::Scheme;
        let mut app = app();
        let first = &mut app.demo.queues[0].commands[0];
        first.title = Some("Server logs".into());
        first.scheme = Some(Scheme::Forest);
        first.command = "export NYSOS_STYLE_TEST=preserved".into();
        let mut next = app.demo.queues[0].clone();
        next.commands.truncate(1);
        next.commands[0].title = Some("Server health".into());
        next.commands[0].scheme = Some(Scheme::Mono);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session");
        next.commands[0].command =
            format!("printf '%s' \"$NYSOS_STYLE_TEST\" > '{}'", path.display());
        app.demo.queues.push(next);
        app.cues.focused = true;
        key(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(app.panes[0].config.display_title(), "presenter");
        key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        app.step(false).unwrap();
        assert_eq!(app.panes[0].config.id, "presenter");
        assert_eq!(app.panes[0].config.display_title(), "Server logs");
        assert_eq!(app.panes[0].config.scheme, Scheme::Forest);
        assert_eq!(app.demo.panes[0].display_title(), "presenter");
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains("Server logs"));
        app.step(true).unwrap(); // Skip observer; no effect on presenter styling.
        app.cues.selected = 1;
        app.execute_selected().unwrap();
        assert_eq!(app.panes[0].config.display_title(), "Server health");
        assert_eq!(app.panes[0].config.scheme, Scheme::Mono);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::fs::read_to_string(&path).ok().as_deref() != Some("preserved") {
            assert!(
                std::time::Instant::now() < deadline,
                "styling replaced the shell session"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app.apply(app.demo.clone()).unwrap();
        assert_eq!(app.panes[0].config.display_title(), "presenter");
        app.step(true).unwrap(); // Skipping an explicitly styled command does not apply it.
        assert_eq!(app.panes[0].config.scheme, Scheme::Ocean);
        app.cues.selected = 1;
        app.type_selected().unwrap();
        assert_eq!(app.panes[0].config.display_title(), "Server health");
        assert_eq!(app.panes[0].config.scheme, Scheme::Mono);
        assert_eq!((app.queue, app.command), (0, 1));
    }
    #[test]
    fn key_diagnostic_shows_received_control_and_active_popup() {
        let mut app = app();
        app.debug_keys = true;
        app.edit_demo().unwrap();
        key(&mut app, KeyCode::Char('l'), KeyModifiers::CONTROL);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let row: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 29)].symbol())
            .collect();
        assert!(row.contains("Char('l')"));
        assert!(row.contains("CONTROL"));
        assert!(row.contains("Mode: load path"));
    }
    #[test]
    fn ctrl_g_controls_demo_without_claiming_ctrl_space() {
        let mut app = app();
        key(&mut app, KeyCode::Char(' '), KeyModifiers::CONTROL);
        assert!(!app.prefix);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(app.prefix);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(!app.prefix);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(app.help);
    }
    #[test]
    fn alternate_prefixes_and_literal_prefix_forwarding() {
        use crate::prefix::Prefix;
        let mut app = app();
        key(&mut app, KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert!(!app.prefix);
        for (prefix, code, modifiers) in [
            (Prefix::CtrlA, KeyCode::Char('a'), KeyModifiers::CONTROL),
            (Prefix::CtrlB, KeyCode::Char('b'), KeyModifiers::CONTROL),
            (Prefix::F12, KeyCode::F(12), KeyModifiers::NONE),
        ] {
            app.demo.prefix = prefix;
            key(&mut app, code, modifiers);
            assert!(app.prefix);
            key(&mut app, code, modifiers);
            assert!(!app.prefix);
            assert_eq!(app.panes.len(), 2); // Ctrl-A twice must not invoke add-pane.
            key(&mut app, code, modifiers);
            key(&mut app, KeyCode::Char('0'), KeyModifiers::NONE);
            assert!(app.cues.focused);
            key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        }
    }
    #[test]
    fn queue_skip_edit_and_completion() {
        let mut app = app();
        app.step(true).unwrap();
        assert_eq!((app.queue, app.command), (0, 1));
        let queue = Queue {
            name: "edited".into(),
            description: "changed".into(),
            commands: vec![DemoCommand {
                pane: "observer".into(),
                command: "printf edited".into(),
                ..Default::default()
            }],
        };
        app.editor = Some(Editor::new(
            EditorKind::Queue(0),
            toml::to_string(&queue).unwrap(),
        ));
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(app.editor.is_none());
        assert_eq!((app.queue, app.command), (0, 0));
        assert_eq!(app.demo.queues[0].name, "edited");
        app.step(false).unwrap();
        assert_eq!((app.queue, app.command), (1, 0));
        app.step(false).unwrap(); // end is idempotent
    }
    #[test]
    fn typing_waits_for_enter_and_allows_editing_in_each_shell() {
        let mut app = app();
        let dir = tempfile::tempdir().unwrap();
        let paths = [dir.path().join("first"), dir.path().join("second")];
        app.demo.queues[0].commands = ["presenter", "observer"]
            .into_iter()
            .zip(&paths)
            .map(|(pane, path)| DemoCommand {
                pane: pane.into(),
                command: format!("printf original > '{}'", path.display()),
                ..Default::default()
            })
            .collect();
        app.cues.focused = true;
        key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(!app.cues.focused);
        assert_eq!(app.active, 0);
        assert_eq!((app.queue, app.command), (0, 0));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(paths.iter().all(|path| !path.exists()));
        for (index, path) in paths.iter().enumerate() {
            // Replace the prepared line using the shell's own line editor.
            app.panes[index]
                .write(format!("\x15printf edited > '{}'\r", path.display()).as_bytes())
                .unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while paths
            .iter()
            .any(|path| std::fs::read_to_string(path).ok().as_deref() != Some("edited"))
        {
            assert!(
                std::time::Instant::now() < deadline,
                "typed lines were not editable in both shells"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!((app.queue, app.command), (0, 0));
    }
    #[test]
    fn cue_preview_tracks_remaining_commands_and_never_executes() {
        let mut app = app();
        app.demo.queues[0].commands = vec![
            DemoCommand {
                pane: "presenter".into(),
                command: "echo preview-first".into(),
                ..Default::default()
            },
            DemoCommand {
                pane: "observer".into(),
                command: "echo preview-second".into(),
                ..Default::default()
            },
        ];
        app.demo.queues.push(app.demo.queues[0].clone());
        app.demo.queues[1].commands.pop();
        app.demo.validate().unwrap();
        app.cues.focused = true;
        assert_eq!(
            app.pane_preview("presenter"),
            (Some("echo preview-first"), 1)
        );
        assert_eq!(
            app.pane_preview("observer"),
            (Some("echo preview-second"), 1)
        );
        app.step(true).unwrap();
        assert_eq!(app.pane_preview("presenter"), (None, 0));
        app.cues.select(1, 2);
        assert_eq!(
            app.pane_preview("presenter"),
            (Some("echo preview-first"), 1)
        );
        key(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.preview);
        key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        app.event(Event::Paste("do not send".into()));
        assert_eq!((app.queue, app.command, app.cues.selected), (0, 1, 1));
        for (width, height) in [(120, 30), (8, 4), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            app.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            if width == 120 {
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(text.contains("echo preview-first"));
                assert!(text.contains("No pending command for this cue"));
            }
        }
        key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.preview);
        assert!(app.cues.focused);
        assert_eq!((app.queue, app.command), (0, 1));
        app.demo.queues.clear();
        key(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(!app.preview);
    }
    #[test]
    fn cue_navigation_edit_execution_and_visibility() {
        let mut app = app();
        let mut second = app.demo.queues[0].clone();
        second.name = "Second cue".into();
        app.demo.queues.push(second);
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        assert_eq!(app.rects[0].x, 26);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('0'), KeyModifiers::NONE);
        assert!(app.cues.focused);
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.cues.selected, 1);
        assert_eq!((app.queue, app.command), (0, 0));
        key(&mut app, KeyCode::Char('e'), KeyModifiers::NONE);
        assert!(matches!(
            app.editor.as_ref().unwrap().kind,
            EditorKind::Queue(1)
        ));
        let mut edited = app.demo.queues[1].clone();
        edited.name = "Edited second".into();
        app.editor = Some(Editor::new(
            EditorKind::Queue(1),
            toml::to_string(&edited).unwrap(),
        ));
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(app.demo.queues[1].name, "Edited second");
        assert_ne!(app.demo.queues[0].name, "Edited second");
        assert_eq!((app.queue, app.command), (0, 0));
        key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!((app.queue, app.command), (2, 0));
        assert!(app.cues.focused);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(!app.demo.cue_list && !app.cues.focused);
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        assert_eq!(app.rects[0].x, 0);
        app.toggle_cues();
        app.active = app.panes.len() - 1;
        app.rotate(1);
        assert!(app.cues.focused);
        app.rotate(1);
        assert!(!app.cues.focused);
        assert_eq!(app.active, 0);
    }
    #[test]
    fn cue_enter_resumes_remaining_commands_in_target_shells() {
        let mut app = app();
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        let output = |path: &std::path::Path, value: &str| {
            format!(
                "printf '{value}' >> '{}'",
                path.display().to_string().replace('\'', "'\\''")
            )
        };
        app.demo.queues[0].commands = vec![
            DemoCommand {
                pane: "presenter".into(),
                command: output(&first, "skipped"),
                ..Default::default()
            },
            DemoCommand {
                pane: "observer".into(),
                command: output(&second, "sent"),
                ..Default::default()
            },
        ];
        app.demo.validate().unwrap();
        app.step(true).unwrap();
        app.cues.focused = true;
        key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!((app.queue, app.command), (1, 0));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if !first.exists() && std::fs::read_to_string(&second).ok().as_deref() == Some("sent") {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("remaining cue command did not reach its target shell");
    }
    #[test]
    fn cue_mouse_selection_and_empty_list() {
        let mut app = app();
        app.demo.queues.push(app.demo.queues[0].clone());
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: app.cues.area.y + 2,
            modifiers: KeyModifiers::NONE,
        })
        .unwrap();
        assert!(app.cues.focused);
        assert_eq!(app.cues.selected, 1);
        assert_eq!(app.queue, 0);
        app.demo.queues.clear();
        for code in [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::End,
            KeyCode::Enter,
            KeyCode::Char('e'),
        ] {
            key(&mut app, code, KeyModifiers::NONE);
        }
        assert!(app.editor.is_none());
        assert_eq!(app.cues.selected, 0);
    }
    #[test]
    fn failed_apply_preserves_live_demo_and_invalid_editor_stays_open() {
        let mut app = app();
        let mut bad = app.demo.clone();
        bad.panes[0].shell = "/does/not/exist/nysos-shell".into();
        assert!(app.apply(bad).is_err());
        assert_eq!(app.demo.panes[0].shell, crate::config::test_shell());
        assert_eq!(app.panes.len(), 2);
        app.editor = Some(Editor::new(EditorKind::Demo, "invalid toml!".into()));
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(!app.editor.as_ref().unwrap().error.is_empty());
        app.editor = Some(Editor::new(EditorKind::Demo, "unsupported = true".into()));
        key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
        let editor = app.editor.as_ref().unwrap();
        assert!(matches!(editor.kind, EditorKind::Demo));
        assert!(
            editor
                .error
                .lines()
                .next()
                .unwrap()
                .contains("unknown field")
        );
    }
    #[test]
    fn mouse_focus_drag_add_and_small_terminal_render() {
        let mut app = app();
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        let right = app.rects[1];
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: right.x + 2,
            row: right.y + 2,
            modifiers: KeyModifiers::NONE,
        })
        .unwrap();
        assert_eq!(app.active, 1);
        let left = app.rects[0];
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: left.right() - 1,
            row: left.y + 3,
            modifiers: KeyModifiers::NONE,
        })
        .unwrap();
        app.mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 70,
            row: left.y + 3,
            modifiers: KeyModifiers::NONE,
        })
        .unwrap();
        assert!(app.demo.panes[0].weight > app.demo.panes[1].weight);
        app.add_pane().unwrap();
        assert_eq!((app.panes.len(), app.active), (3, 2));
        for (width, height) in [(100, 30), (8, 4), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            app.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
        }
    }
    #[test]
    fn modal_save_and_load_round_trip() {
        let mut app = app();
        let dir = tempfile::tempdir().unwrap();
        app.path = dir.path().join("saved.toml");
        app.demo.header = false;
        app.demo.title = "Full demo title".into();
        let mut added = app.demo.queues[0].clone();
        added.name = "Added cue".into();
        app.demo.queues.push(added);
        app.cues.focused = true;
        key(&mut app, KeyCode::Char('o'), KeyModifiers::NONE);
        assert!(matches!(
            app.editor.as_ref().unwrap().kind,
            EditorKind::Demo
        ));
        assert!(app.editor.as_ref().unwrap().content().contains("Added cue"));
        key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.editor.is_none());
        assert!(!Demo::load(&app.path).unwrap().header);
        app.demo.header = true;
        app.demo.title = "Unsaved title".into();
        app.demo.queues.pop();
        app.editor = Some(Editor::new(
            EditorKind::Demo,
            toml::to_string(&app.demo).unwrap(),
        ));
        key(&mut app, KeyCode::Char('l'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.demo.header); // Load previews; it does not apply or execute.
        assert_eq!(app.demo.title, "Unsaved title");
        assert_eq!(app.demo.queues.len(), 1);
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(!app.demo.header);
        assert_eq!(app.demo.title, "Full demo title");
        assert_eq!(app.demo.queues[1].name, "Added cue");
        assert_eq!((app.queue, app.command), (0, 0));
    }
}
