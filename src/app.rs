// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use crate::{
    config::{Axis, Demo, PaneConfig, Queue},
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
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
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

#[derive(Clone)]
struct LayoutSnapshot {
    layout: crate::config::Layout,
    weights: Vec<u16>,
}
impl LayoutSnapshot {
    fn capture(demo: &Demo, cue: Option<usize>) -> Self {
        Self {
            layout: demo.layout_for(cue).clone(),
            weights: demo.panes.iter().map(|p| p.weight).collect(),
        }
    }
    fn configs(&self, demo: &Demo) -> Vec<PaneConfig> {
        let mut configs = demo.panes.clone();
        for (config, weight) in configs.iter_mut().zip(&self.weights) {
            config.weight = *weight;
        }
        configs
    }
}

pub struct App {
    pub demo: Demo,
    pub panes: Vec<Pane>,
    pub active: usize,
    queue: usize,
    command: usize,
    layout_cue: Option<usize>,
    layout_applied: Option<usize>,
    cue_layouts: HashMap<usize, LayoutSnapshot>,
    // Historical browsing never mutates the live layout or its weights.
    viewed_layout: Option<LayoutSnapshot>,
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
    viewport: Rect,
    drag: Option<layout::Divider>,
    dividers: Vec<layout::Divider>,
    cues: CueList,
    advance: Option<(Instant, usize)>,
    timer_paused: bool,
    last_tick: Instant,
}
impl App {
    pub fn new(demo: Demo, path: PathBuf) -> Result<Self> {
        demo.validate()?;
        let mut panes: Vec<Pane> = demo
            .panes
            .iter()
            .cloned()
            .map(Pane::spawn)
            .collect::<Result<_>>()?;
        for pane in &mut panes {
            pane.configure_line_numbers(
                demo.features
                    .contains(&crate::features::Feature::LineNumbers),
                demo.line_numbers,
            );
        }
        let detected_terminal = crate::prefix::TerminalKeys::detect(
            std::env::var("TERM_PROGRAM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
        );
        let terminal_label = demo.terminal_keys.resolve(detected_terminal).label();
        let mut cues = CueList::default();
        cues.focused = demo.cue_list;
        Ok(Self {
            demo,
            panes,
            active: 0,
            queue: 0,
            command: 0,
            layout_cue: None,
            layout_applied: None,
            cue_layouts: HashMap::new(),
            viewed_layout: None,
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
            viewport: Rect::default(),
            drag: None,
            dividers: vec![],
            cues,
            advance: None,
            timer_paused: false,
            last_tick: Instant::now(),
        })
    }
    pub fn tick(&mut self) -> Result<()> {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        if let Some((deadline, _)) = &mut self.advance
            && (self.timer_paused || self.editor.is_some() || self.help || self.preview)
        {
            *deadline += elapsed;
        }
        for pane in &mut self.panes {
            pane.pump()?;
        }
        if self.advance.is_some_and(|(deadline, _)| now >= deadline)
            && !self.timer_paused
            && self.editor.is_none()
            && !self.help
            && !self.preview
        {
            let (_, next) = self.advance.take().unwrap();
            self.cues.select(next, self.demo.queues.len());
            if let Err(error) = self.execute_selected() {
                self.advance = None;
                self.status = format!("Automatic playback stopped: {error}");
            }
        }
        Ok(())
    }
    pub fn resize(&mut self, area: Rect) -> Result<()> {
        if self.viewport != area {
            self.drag = None;
            self.viewport = area;
        }
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
        let geometry = if let Some(snapshot) = &self.viewed_layout {
            layout::panes(
                horizontal[1],
                &snapshot.layout,
                &snapshot.configs(&self.demo),
            )
        } else {
            layout::panes(
                horizontal[1],
                self.demo.layout_for(self.layout_cue),
                &self.demo.panes,
            )
        };
        self.rects = geometry.panes;
        self.dividers = geometry.dividers;
        if !self.pane_visible(self.active) {
            self.active = (0..self.panes.len())
                .find(|&i| self.pane_visible(i))
                .unwrap_or(0);
        }
        for (pane, rect) in self.panes.iter_mut().zip(&self.rects) {
            pane.configure_line_numbers(
                self.demo
                    .features
                    .contains(&crate::features::Feature::LineNumbers),
                self.demo.line_numbers,
            );
            // Hidden sessions keep their terminal size, history, and running processes.
            if rect.width > 0 && rect.height > 0 {
                pane.resize(pane.body_area(inner(*rect)))?;
            }
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
            let text = if let Some(queue) = self.demo.queues.get(queue_index) {
                format!(
                    "{}/{} · {} · {} {}{}\n{}",
                    queue_index + 1,
                    self.demo.queues.len(),
                    queue.name,
                    queue.commands.len(),
                    if queue.commands.iter().all(|a| !a.command.is_empty()) {
                        "command"
                    } else {
                        "action"
                    },
                    if queue.commands.len() == 1 { "" } else { "s" },
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
            if area.width == 0 || area.height == 0 {
                continue;
            }
            let (fg, bg, accent) = palette(pane.config.scheme);
            let title = format!(
                " {} · {}{} ",
                i + 1,
                pane.config.display_title(),
                if pane.exited { " [exited]" } else { "" }
            );
            frame.render_widget(
                Block::bordered()
                    .title(Line::styled(
                        title,
                        Style::default()
                            .fg(fg)
                            .bg(bg)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ))
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
        let playback = self.advance.map(|(deadline, _)| {
            format!(
                "Auto: {:.1}s{}{} · Space pauses in Cues",
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs_f32(),
                if self.timer_paused || self.editor.is_some() || self.help || self.preview {
                    " (paused)"
                } else {
                    ""
                },
                if self.demo.loop_cues { " · loop" } else { "" }
            )
        });
        let status = if self.prefix {
            "PREFIX: n run · s skip · e edit cue · o edit demo/add cues · a add · Tab focus · c cues · 0 focus cues · <> width · -+ height · l layout · h header · # numbers · x restart · q quit · ? help"
        } else if let Some(playback) = &playback {
            playback
        } else {
            &self.status
        };
        let hints = if self.preview {
            "PREVIEW: ↑↓ scroll · Esc close · no commands executed"
        } else if self.cues.focused {
            "CUES: ↑↓ select · Enter run · p preview · t type · s bookmark · b bottom · Space pause · e edit · o demo"
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
                let body = pane.body_area(inner(*rect));
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
                let text = self
                    .pane_action(&pane.config.id)
                    .map(|action| action.preview())
                    .unwrap_or_else(|| "No pending action for this cue".into());
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
        if matches!(
            event,
            Event::Key(_) | Event::Resize(_, _) | Event::FocusLost
        ) {
            self.drag = None;
        }
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
                        KeyCode::Char(c @ ('<' | '>' | '-' | '+')) => {
                            let (axis, delta) = match c {
                                '<' => (Axis::Columns, -2),
                                '>' => (Axis::Columns, 2),
                                '-' => (Axis::Rows, -1),
                                _ => (Axis::Rows, 1),
                            };
                            let dividers = self.dividers.clone();
                            let rect = self.rects.get(self.active).copied();
                            let changed = !self.cues.focused
                                && rect.is_some_and(|rect| {
                                    self.edit_display_layout(|demo, cue| {
                                        layout::resize_focused(
                                            demo, cue, &dividers, rect, axis, delta,
                                        )
                                    })
                                });
                            self.status = if changed && self.viewed_layout.is_some() { "Historical view resized · b restores the live layout" } else if changed { "Pane resized · save the full demo to keep its layout" } else { "Focus a shell with a resizable split on that axis; use prefix o to edit layout" }.into();
                        }
                        KeyCode::Char('#') => self.toggle_line_numbers()?,
                        KeyCode::Char('h') => self.demo.header = !self.demo.header,
                        KeyCode::Char('l') => {
                            self.edit_display_layout(|demo, cue| demo.layout_for_mut(cue).cycle());
                            self.drag = None;
                        }
                        KeyCode::Char('x') if !self.cues.focused => {
                            self.panes[self.active] =
                                Pane::spawn(self.panes[self.active].config.clone())?
                        }
                        KeyCode::Char('?') => self.help = true,
                        KeyCode::Char(c @ '1'..='9') => {
                            let i = (c as u8 - b'1') as usize;
                            if i < self.panes.len() && self.pane_visible(i) {
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
        let count = self.panes.len() + extra;
        let next = (1..=count)
            .map(|step| {
                (position as isize + direction * step as isize).rem_euclid(count as isize) as usize
            })
            .find(|&next| (extra == 1 && next == 0) || self.pane_visible(next - extra))
            .unwrap_or(position);
        self.cues.focused = extra == 1 && next == 0;
        if !self.cues.focused {
            self.active = next - extra;
        }
    }
    fn pane_visible(&self, index: usize) -> bool {
        self.viewed_layout
            .as_ref()
            .map_or_else(
                || self.demo.layout_for(self.layout_cue),
                |snapshot| &snapshot.layout,
            )
            .contains_pane(&self.panes[index].config.id)
    }
    fn edit_display_layout<T>(&mut self, edit: impl FnOnce(&mut Demo, Option<usize>) -> T) -> T {
        if let Some(snapshot) = &self.viewed_layout {
            let mut demo = self.demo.clone();
            demo.layout = snapshot.layout.clone();
            demo.panes = snapshot.configs(&self.demo);
            let result = edit(&mut demo, None);
            self.viewed_layout = Some(LayoutSnapshot::capture(&demo, None));
            result
        } else {
            edit(&mut self.demo, self.layout_cue)
        }
    }
    fn return_to_live(&mut self) -> Result<()> {
        self.viewed_layout = None;
        self.drag = None;
        self.resize(self.viewport)?;
        for pane in &mut self.panes {
            pane.term.scroll_display(Scroll::Bottom);
        }
        Ok(())
    }
    fn toggle_line_numbers(&mut self) -> Result<()> {
        if !self
            .demo
            .features
            .contains(&crate::features::Feature::LineNumbers)
        {
            self.status = "Line numbers are gated · enable features = [\"line-numbers\"] in the full demo editor".into();
        } else if self.cues.focused {
            self.status = "Focus a shell pane before toggling line numbers".into();
        } else {
            let show = !self.panes[self.active]
                .config
                .line_numbers
                .unwrap_or(self.demo.line_numbers);
            self.panes[self.active].config.line_numbers = Some(show);
            self.demo.panes[self.active].line_numbers = Some(show);
            self.resize(self.viewport)?;
            self.status = format!(
                "Line numbers {} · save the full demo to keep this pane default",
                if show { "shown" } else { "hidden" }
            );
        }
        Ok(())
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
    fn pane_action(&self, pane: &str) -> Option<&crate::config::Command> {
        let cue = self.demo.queues.get(self.cues.selected)?;
        let start = if self.cues.selected == self.queue {
            self.command
        } else {
            0
        };
        cue.commands
            .iter()
            .skip(start)
            .find(|action| action.pane == pane)
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
        self.advance = None;
        let commands: Vec<_> = self
            .panes
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                self.pane_action(&pane.config.id)
                    .cloned()
                    .map(|action| (index, action))
            })
            .collect();
        let Some((first, _)) = commands.first() else {
            return Ok(());
        };
        // Without shell text to edit, typing has exactly the same effect as Enter.
        if commands.iter().all(|(_, action)| action.command.is_empty()) {
            return self.execute_selected();
        }
        for (index, action) in &commands {
            if action.command.chars().any(char::is_control) {
                bail!("Typing requires commands without control characters (including tabs)");
            }
            self.panes[*index].pump()?;
            if self.panes[*index].exited && !action.clear {
                bail!("Pane '{}' has exited", action.pane);
            }
        }
        for (index, action) in &commands {
            if let Some(show) = action.line_numbers {
                self.panes[*index].config.line_numbers = Some(show);
            }
        }
        self.apply_cue_layout(self.cues.selected)?;
        self.resize(self.viewport)?;
        for (index, action) in &commands {
            let pane = &mut self.panes[*index];
            pane.bookmark(self.cues.selected);
            pane.send_action(action, true)?;
            pane.config.apply_command_style(action);
        }
        if self.pane_visible(*first) {
            self.active = *first;
        }
        self.cues.focused = false;
        self.status =
            "Typed without Enter · key/clear actions sent immediately · cue progress unchanged"
                .into();
        Ok(())
    }
    fn apply_cue_layout(&mut self, index: usize) -> Result<()> {
        if self.viewed_layout.is_some() {
            self.return_to_live()?;
        }
        if self.demo.queues[index].layout.is_some() {
            self.layout_cue = Some(index);
            self.drag = None;
            self.resize(self.viewport)?;
        }
        self.cue_layouts
            .insert(index, LayoutSnapshot::capture(&self.demo, self.layout_cue));
        // Include visible panes without an action, so the whole historical view is useful.
        for i in 0..self.panes.len() {
            if self.pane_visible(i) {
                self.panes[i].pump()?;
                self.panes[i].bookmark(index);
            }
        }
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
        if matches!(
            key.code,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            self.advance = None;
        }
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
            KeyCode::Char(' ') => {
                if self.advance.is_some() {
                    self.timer_paused = !self.timer_paused;
                } else {
                    self.status = "No timer armed · run a cue with advance_after_ms".into();
                }
            }
            KeyCode::Char('s') => {
                self.advance = None;
                if let Some(snapshot) = self.cue_layouts.get(&self.cues.selected) {
                    self.viewed_layout = Some(snapshot.clone());
                    self.drag = None;
                    self.resize(self.viewport)?;
                } else {
                    self.status = "No recorded layout or bookmarks for this cue".into();
                    return Ok(());
                }
                let mut restored = 0;
                let mut targets = 0;
                if let Some(cue) = self.demo.queues.get(self.cues.selected) {
                    for pane in &mut self.panes {
                        if self
                            .viewed_layout
                            .as_ref()
                            .unwrap()
                            .layout
                            .contains_pane(&pane.config.id)
                            || cue
                                .commands
                                .iter()
                                .any(|action| action.pane == pane.config.id)
                        {
                            targets += 1;
                            restored += usize::from(pane.restore_bookmark(self.cues.selected));
                        }
                    }
                }
                self.status = format!(
                    "Restored {restored}/{targets} pane bookmarks · viewing cue {} layout · b returns live",
                    self.cues.selected + 1,
                );
                if restored < targets {
                    self.status
                        .push_str(" · missing bookmarks were not recorded or have expired");
                }
            }
            KeyCode::Char('b') => {
                self.return_to_live()?;
                self.status.clear();
            }
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
        self.advance = None;
        self.timer_paused = false;
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
            if pane.exited && !command.clear {
                bail!(
                    "Pane '{}' has exited; focus it and use {} x before running this cue",
                    command.pane,
                    self.demo.prefix.label()
                );
            }
        }
        let remaining = cue.commands.len() - start;
        if start == 0 {
            self.layout_applied = None;
        }
        self.queue = index;
        self.command = start;
        for _ in 0..remaining {
            self.step(false)?;
        }
        self.cues.select(self.queue, self.demo.queues.len());
        Ok(())
    }
    fn step(&mut self, skip: bool) -> Result<()> {
        self.advance = None;
        let Some(queue) = self.demo.queues.get(self.queue) else {
            self.status = "Demo complete".into();
            return Ok(());
        };
        let command = queue.commands[self.command].clone();
        let command_count = queue.commands.len();
        let delay = queue.advance_after_ms;
        if !skip {
            if self.viewed_layout.is_some() {
                self.return_to_live()?;
            }
            if self.layout_applied != Some(self.queue) {
                self.apply_cue_layout(self.queue)?;
                self.layout_applied = Some(self.queue);
            }
            if let Some(show) = command.line_numbers {
                self.panes
                    .iter_mut()
                    .find(|p| p.config.id == command.pane)
                    .expect("validated target")
                    .config
                    .line_numbers = Some(show);
                self.resize(self.viewport)?;
            }
            let pane = self
                .panes
                .iter_mut()
                .find(|p| p.config.id == command.pane)
                .expect("validated target");
            pane.pump()?;
            pane.bookmark(self.queue);
            pane.send_action(&command, false)?;
            pane.config.apply_command_style(&command);
        }
        self.status.clear();
        self.command += 1;
        if self.command == command_count {
            self.queue += 1;
            if self.queue == self.demo.queues.len() && self.demo.loop_cues {
                self.queue = 0;
            }
            if !skip && self.queue < self.demo.queues.len() {
                self.advance =
                    delay.map(|ms| (Instant::now() + Duration::from_millis(ms), self.queue));
                self.last_tick = Instant::now();
            }
            self.command = 0;
            self.layout_applied = None;
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
        self.return_to_live()?;
        let name = (1..)
            .map(|n| format!("adhoc-{n}"))
            .find(|n| !self.demo.panes.iter().any(|p| p.id == *n))
            .unwrap();
        let config = PaneConfig {
            id: name,
            ..Default::default()
        };
        let pane = Pane::spawn(config.clone())?;
        self.demo.layout.add_pane(&config.id);
        for cue in &mut self.demo.queues {
            if let Some(layout) = &mut cue.layout {
                layout.add_pane(&config.id);
            }
        }
        self.drag = None;
        self.demo.panes.push(config);
        self.panes.push(pane);
        self.cue_layouts.clear();
        for pane in &mut self.panes {
            pane.forget_bookmarks();
        }
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
        self.advance = None;
        self.timer_paused = false;
        for pane in &mut self.panes {
            pane.forget_bookmarks();
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
        self.layout_cue = None;
        self.layout_applied = None;
        self.cue_layouts.clear();
        self.viewed_layout = None;
        self.drag = None;
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
                        self.advance = None;
                        for pane in &mut self.panes {
                            pane.forget_bookmarks();
                        }
                        self.cue_layouts.clear();
                        self.viewed_layout = None;
                        self.drag = None;
                        self.layout_applied = None;
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
            && let Some(divider) = &self.drag
        {
            let divider = divider.clone();
            self.edit_display_layout(|demo, cue| layout::resize_pair(demo, cue, &divider, x, y));
            return Ok(());
        }
        if self.demo.cue_list && self.cues.area.contains((x, y).into()) {
            if matches!(
                event.kind,
                MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            ) {
                self.advance = None;
            }
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
        let outer_body = inner(rect);
        let body = self.panes[index].body_area(outer_body);
        if outer_body.contains((x, y).into()) && x < body.x {
            match event.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.active = index;
                    self.cues.focused = false;
                }
                MouseEventKind::ScrollUp => self.panes[index].term.scroll_display(Scroll::Delta(3)),
                MouseEventKind::ScrollDown => {
                    self.panes[index].term.scroll_display(Scroll::Delta(-3))
                }
                _ => {}
            }
            return Ok(());
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            self.active = index;
            self.cues.focused = false;
            if !body.contains((x, y).into()) {
                self.drag = self
                    .dividers
                    .iter()
                    .rev()
                    .find(|d| d.contains(x, y))
                    .cloned();
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
const HELP: &str = "Every pane is a live PTY shell. Type normally; Ctrl-C reaches the shell.\n\nPress Ctrl-G, release, then:\n  n / Enter    Send the next command and advance\n  s            Skip the next command\n  e            Edit current queue item before running it\n  o            Edit full demo (title, add/reorder cues, panes, layout)\n  a            Add and focus an ad hoc shell\n  Tab / →      Focus next pane; Shift-Tab / ← goes back\n  0            Show and focus the cue list\n  c            Show/hide the cue list\n  1–9          Focus shell pane by number\n  l / h        Cycle layout / toggle header\n  #            Toggle focused pane line numbers (line-numbers gate)\n  x            Restart focused shell (ends its current session)\n  q            Quit and close all shells\n\nIn Cues: ↑/↓ browse, Enter sends the selected cue, e edits it; o edits the entire demo.\np previews the next command/key/clear action per pane; Esc closes, arrows scroll.\nt types commands without Enter; keys and native clears are sent immediately.\ns restores the selected cue’s recorded layout and pane bookmarks; b restores the live layout and bottom.\nSpace pauses/resumes an armed timer. Browsing or s cancels automatic playback.\nTab/Esc returns to a shell. Enter resumes a partially sent current cue.\nCommands are dispatched in order without waiting for completion.\n\nAlt-Left/Right rotates focus, including Cues. Ghostty mappings also accept Alt-B/F. Click a pane to focus.\nDrag a shared border to resize nested groups. Prefix < / > changes width; - / + changes height. Grid rows are equal height.\nScroll wheel uses scrollback; Shift-wheel overrides application mouse mode.\nAlt-click opens HTTP(S) links. Cmd-click works locally on macOS outside tmux when the host forwards the click.\nOver SSH, URL openers run on the remote host.\n\nEditor: Ctrl-L load, Ctrl-S save as, Ctrl-G apply, Esc cancel.\nLoading previews the file; applying resets queue progress. Changing a pane\nid/shell/args/cwd creates a new session. Removed sessions are closed.\nCommands are sent to the pane's current foreground program: wait for its\nprompt before running the next command. No automatic completion detection.";

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::config::Command as DemoCommand;
    use ratatui::{Terminal, backend::TestBackend};
    fn app() -> App {
        let mut demo = crate::config::test_demo();
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
    fn historical_layouts_preserve_hidden_sessions_and_the_live_arrangement() {
        use crate::config::{Layout, LayoutNode, LayoutPreset};
        use alacritty_terminal::grid::Dimensions;
        let mut app = app();
        for command in &mut app.demo.queues[0].commands {
            command.command = ":".into();
        }
        // A pane with no action still needs a bookmark in the historical view.
        app.demo.queues[0].commands.truncate(1);
        let initial = app.demo.queues[0].clone();
        let mut hidden = initial.clone();
        hidden.layout = Some(Layout::Tree(LayoutNode::Pane {
            pane: "presenter".into(),
            weight: 1,
        }));
        let mut rows = initial.clone();
        rows.layout = Some(Layout::Preset(LayoutPreset::Rows));
        app.demo.queues = vec![initial.clone(), hidden, initial.clone(), rows, initial];
        app.demo.validate().unwrap();
        let viewport = Rect::new(0, 0, 120, 40);
        app.resize(viewport).unwrap();
        let initial_rects = app.rects.clone();
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        app.panes[1]
            .write(
                format!(
                    "remembered=still_alive; printf ready > '{}'\n",
                    ready.display()
                )
                .as_bytes(),
            )
            .unwrap();
        let wait_for = |app: &mut App, path: &std::path::Path, expected: &str| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.tick().unwrap();
                if std::fs::read_to_string(path).unwrap_or_default() == expected {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "shell did not acknowledge command"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        wait_for(&mut app, &ready, "ready");
        app.execute_selected().unwrap();
        let hidden_size = (
            app.panes[1].term.columns(),
            app.panes[1].term.screen_lines(),
        );
        app.active = 1;
        app.execute_selected().unwrap();
        assert_eq!(app.rects[1], Rect::default());
        assert_eq!(app.active, 0);
        assert_eq!(
            (
                app.panes[1].term.columns(),
                app.panes[1].term.screen_lines()
            ),
            hidden_size
        );
        app.cues.focused = false;
        app.rotate(1);
        assert!(app.cues.focused); // Focus skips the hidden observer.
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
        assert!(app.cues.focused);
        let done = dir.path().join("done");
        app.panes[1].write(format!("i=0; while [ $i -lt 60 ]; do printf 'hidden output %s\\n' \"$i\"; i=$((i+1)); done; printf %s \"$remembered\" > '{}'\n", done.display()).as_bytes()).unwrap();
        wait_for(&mut app, &done, "still_alive");
        // Pump bytes written immediately before the acknowledgement as well.
        app.tick().unwrap();
        assert!(app.panes[1].term.history_size() > 0);
        app.execute_selected().unwrap(); // Inherits the one-pane layout.
        let hidden_rects = app.rects.clone();
        app.execute_selected().unwrap(); // Both panes visible in rows.
        app.execute_selected().unwrap(); // Inherits rows.
        let rows_rects = app.rects.clone();
        app.demo.panes[0].weight = 3; // Manual live resize after the last cue.
        app.resize(viewport).unwrap();
        let live_rects = app.rects.clone();
        assert_ne!(live_rects, rows_rects);
        let progress = (app.queue, app.command);
        for (cue, expected) in [
            (0, &initial_rects),
            (2, &hidden_rects),
            (4, &rows_rects),
            (1, &hidden_rects),
            (0, &initial_rects),
        ] {
            app.cues.select(cue, 5);
            app.cue_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))
                .unwrap();
            assert_eq!(&app.rects, expected);
            assert_eq!((app.queue, app.command), progress);
            assert_eq!(app.demo.panes[0].weight, 3);
        }
        assert!(app.status.contains("Restored 2/2"), "{}", app.status);
        assert!(app.panes[1].term.grid().display_offset() > 0);
        // Resizing a historical view must not edit either the saved cue or live view.
        app.edit_display_layout(|demo, _| demo.panes[0].weight = 9);
        app.resize(viewport).unwrap();
        assert_ne!(app.rects, initial_rects);
        app.cue_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.rects, initial_rects);
        app.cue_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.rects, live_rects);
        assert!(
            app.panes
                .iter()
                .all(|p| p.term.grid().display_offset() == 0)
        );
        // Dispatch from history inherits the live layout, never the browsed layout.
        app.cues.select(0, 5);
        app.cue_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))
            .unwrap();
        app.cues.select(4, 5);
        app.execute_selected().unwrap();
        assert_eq!(app.rects, live_rects);
        assert!(app.viewed_layout.is_none());
        app.apply(app.demo.clone()).unwrap();
        assert!(app.cue_layouts.is_empty());
    }

    #[test]
    fn typed_layout_bookmarks_and_unexecuted_cues_do_not_replace_live_layout() {
        use crate::config::{Layout, LayoutPreset};
        let mut app = app();
        app.demo.queues[0].commands.truncate(1);
        app.demo.queues[0].commands[0].command = ":".into();
        app.demo.queues.push(app.demo.queues[0].clone());
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        app.type_selected().unwrap();
        let typed_rects = app.rects.clone();
        app.demo.layout = Layout::Preset(LayoutPreset::Rows);
        app.resize(app.viewport).unwrap();
        let live_rects = app.rects.clone();
        app.cues.focused = true;
        key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.rects, typed_rects);
        app.cues.select(1, 2);
        key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.rects, typed_rects);
        assert!(app.status.contains("No recorded layout"));
        key(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(app.rects, live_rects);
    }
    #[test]
    fn cue_bookmarks_survive_demo_layout_changes_and_manual_shell_output() {
        let mut demo = Demo::builtin().unwrap();
        for pane in &mut demo.panes {
            pane.shell = crate::config::test_shell();
            pane.args = crate::config::test_shell_args();
        }
        let mut app = App::new(demo, "demo.toml".into()).unwrap();
        app.resize(Rect::new(0, 0, 120, 32)).unwrap();
        for _ in 0..5 {
            app.execute_selected().unwrap(); // Cue 5 makes the first layout change.
        }
        let dir = tempfile::tempdir().unwrap();
        for index in 0..app.panes.len() {
            app.active = index;
            app.cues.focused = false;
            let done = dir.path().join(format!("pane-{index}"));
            app.event(Event::Paste(format!("i=0; while [ $i -lt 60 ]; do printf 'manual output %s\\n' \"$i\"; i=$((i+1)); done; printf done > '{}'", done.display())));
            key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
            let deadline = Instant::now() + Duration::from_secs(10);
            while std::fs::read_to_string(&done).unwrap_or_default() != "done" {
                app.tick().unwrap();
                assert!(Instant::now() < deadline, "manual command did not complete");
                std::thread::sleep(Duration::from_millis(5));
            }
            app.tick().unwrap();
        }
        app.cues.focused = true;
        app.cues.select(0, app.demo.queues.len());
        key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
        assert!(app.status.contains("Restored 3/3"), "{}", app.status);
        assert!(app.panes.iter().all(|p| p.term.grid().display_offset() > 0));
        key(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.panes
                .iter()
                .all(|p| p.term.grid().display_offset() == 0)
        );
    }
    #[test]
    fn gutter_gate_toggle_mouse_and_cue_visibility_use_the_same_geometry() {
        let mut app = app();
        app.demo.features.clear();
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        app.cues.focused = false;
        let area = inner(app.rects[0]);
        app.toggle_line_numbers().unwrap();
        assert!(app.status.contains("gated"));
        assert_eq!(app.panes[0].body_area(area), area);
        app.demo
            .features
            .push(crate::features::Feature::LineNumbers);
        app.resize(app.viewport).unwrap();
        assert_eq!(app.panes[0].body_area(area), area); // Gate on, initially hidden.
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('#'), KeyModifiers::NONE);
        let body = app.panes[0].body_area(area);
        assert_eq!(body.x, area.x + 8);
        app.cues.focused = true;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + 1,
            row: area.y,
            modifiers: KeyModifiers::ALT,
        })
        .unwrap();
        assert!(!app.cues.focused); // Gutter focuses; it cannot open a URL or forward a click.
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        key(&mut app, KeyCode::Char('#'), KeyModifiers::NONE);
        assert_eq!(app.panes[0].body_area(area), area);
        assert_eq!(app.demo.panes[0].line_numbers, Some(false));
        assert_eq!(
            Demo::parse(&toml::to_string(&app.demo).unwrap())
                .unwrap()
                .panes[0]
                .line_numbers,
            Some(false)
        );
        app.demo.queues[0].commands[0].line_numbers = Some(true);
        app.step(false).unwrap();
        assert_eq!(app.panes[0].body_area(area), body);
        assert!(app.panes[0].restore_bookmark(0));
        app.demo.features.clear();
        app.resize(app.viewport).unwrap();
        assert_eq!(app.panes[0].body_area(area), area);
    }
    #[test]
    fn timers_execute_pause_and_loop_without_recursive_dispatch() {
        let mut app = app();
        for action in &mut app.demo.queues[0].commands {
            action.command.clear();
            action.clear = true;
        }
        app.demo.queues[0].advance_after_ms = Some(1000);
        app.demo.loop_cues = true;
        app.execute_selected().unwrap();
        assert_eq!((app.queue, app.command), (0, 0));
        assert!(app.advance.is_some());
        // An expired one-cue loop dispatches once per frame, not recursively.
        app.advance = Some((Instant::now() - Duration::from_millis(1), 0));
        app.tick().unwrap();
        assert!(app.advance.unwrap().0 > Instant::now());
        app.timer_paused = true;
        app.last_tick = Instant::now();
        app.advance = Some((app.last_tick - Duration::from_millis(1), 0));
        app.tick().unwrap();
        assert!(app.advance.unwrap().0 < Instant::now());
        app.timer_paused = false;
        app.preview = true;
        app.tick().unwrap();
        assert!(app.advance.unwrap().0 < Instant::now());
        app.preview = false;
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
        assert!(app.advance.is_none());
        app.demo.loop_cues = false;
        app.execute_selected().unwrap();
        assert_eq!(app.queue, 1);
        assert!(app.advance.is_none());
        app.execute_selected().unwrap(); // Last cue remains replayable.
        assert_eq!(app.queue, 1);
    }
    #[test]
    fn automatic_transition_executes_next_cue_and_key_typing_advances() {
        let mut app = app();
        app.demo.queues[0].commands = vec![DemoCommand {
            pane: "presenter".into(),
            keys: vec![crate::config::KeyPress {
                key: "Esc".into(),
                repeat: 2,
            }],
            ..Default::default()
        }];
        app.demo.queues[0].advance_after_ms = Some(1000);
        let mut next = app.demo.queues[0].clone();
        next.commands[0].keys.clear();
        next.commands[0].clear = true;
        next.commands[0].title = Some("Timer executed".into());
        next.advance_after_ms = None;
        app.demo.queues.push(next);
        assert_eq!(
            app.pane_action("presenter").unwrap().preview(),
            "Send keys: Esc × 2"
        );
        app.type_selected().unwrap();
        assert_eq!(app.queue, 1);
        app.advance = Some((Instant::now() - Duration::from_millis(1), 1));
        app.tick().unwrap();
        assert_eq!(app.panes[0].config.display_title(), "Timer executed");
        assert_eq!(app.queue, 2);
        assert!(app.advance.is_none());
        key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
        assert!(app.status.contains("Restored 2/2"));
        key(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.panes
                .iter()
                .all(|p| p.term.grid().display_offset() == 0)
        );
    }
    #[test]
    fn builtin_final_cue_replays_on_every_enter() {
        let mut demo = Demo::builtin().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let paths = [
            dir.path().join("service"),
            dir.path().join("observer"),
            dir.path().join("notes"),
        ];
        for pane in &mut demo.panes {
            pane.shell = crate::config::test_shell();
            pane.args = crate::config::test_shell_args();
        }
        for cue in &mut demo.queues {
            for command in &mut cue.commands {
                let index = demo
                    .panes
                    .iter()
                    .position(|p| p.id == command.pane)
                    .unwrap();
                command
                    .command
                    .push_str(&format!("; printf x >> '{}'", paths[index].display()));
            }
        }
        let count = demo.queues.len();
        let mut app = App::new(demo, "demo.toml".into()).unwrap();
        app.cues.focused = true;
        app.resize(Rect::new(0, 0, 100, 30)).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut expected = vec![0; paths.len()];
        for _ in 0..count + 32 {
            for command in &app.demo.queues[app.cues.selected].commands {
                let index = app
                    .demo
                    .panes
                    .iter()
                    .position(|p| p.id == command.pane)
                    .unwrap();
                expected[index] += 1;
            }
            key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
            // Wait for actual shell acknowledgement, not an arbitrary sleep. A
            // burst of dozens of long lines can overflow macOS's PTY input
            // queue before a busy CI shell consumes them. Like a presenter,
            // wait for this cue before sending the next one.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                app.tick().unwrap();
                terminal.draw(|frame| app.draw(frame)).unwrap();
                let lengths: Vec<_> = paths
                    .iter()
                    .map(|path| std::fs::read(path).unwrap_or_default().len())
                    .collect();
                if lengths == expected {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "cue acknowledgement missing: {lengths:?}, expected {expected:?}"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        assert_eq!(
            (app.queue, app.command, app.cues.selected),
            (count, 0, count - 1)
        );
        assert!(app.cues.focused);
        assert_eq!(app.pane_preview("service").1, 1);
        assert_eq!(app.pane_preview("observer").1, 1);
        assert_eq!(expected, vec![44, 37, 38]);
        // Exercise the actual guided tour's bookmark instructions after all of
        // its layout changes and shell output, not just synthetic layouts.
        let live_rects = app.rects.clone();
        for cue in [7, 4, 2, 10, 7] {
            app.cues.select(cue, count);
            key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
            let pane_count = match cue {
                7 => 1,
                4 => 2,
                _ => 3,
            };
            assert!(
                app.status
                    .contains(&format!("Restored {pane_count}/{pane_count}")),
                "cue {}: {}",
                cue + 1,
                app.status
            );
            assert_eq!(app.rects.iter().filter(|r| r.width > 0).count(), pane_count);
            if cue == 7 {
                terminal.draw(|frame| app.draw(frame)).unwrap();
                let cells: String = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(
                    cells.contains("record 001"),
                    "cue 8 must show its original output"
                );
            }
            assert_eq!((app.queue, app.command), (count, 0));
        }
        key(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(app.rects, live_rects);
        assert!(
            app.panes
                .iter()
                .all(|pane| pane.term.grid().display_offset() == 0)
        );
    }
    #[test]
    fn cue_layouts_apply_on_dispatch_and_type_without_overwriting_global_layout() {
        use crate::config::{Layout, LayoutPreset};
        let mut app = app();
        let global = app.demo.layout.clone();
        app.demo.queues[0].layout = Some(Layout::Preset(LayoutPreset::Rows));
        app.demo.queues.push(app.demo.queues[0].clone());
        app.demo.queues[1].layout = Some(global.clone());
        app.cues.focused = true;
        app.resize(Rect::new(0, 0, 120, 40)).unwrap();
        key(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.layout_cue.is_none());
        key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        app.step(true).unwrap();
        assert!(app.layout_cue.is_none());
        app.step(false).unwrap();
        assert_eq!(app.layout_cue, Some(0));
        assert_eq!(app.demo.layout, global);
        assert!(app.rects[1].y > app.rects[0].y);
        app.cues.select(1, 2);
        app.type_selected().unwrap();
        assert_eq!(app.layout_cue, Some(1));
        assert_eq!(app.rects[1].y, app.rects[0].y);
        let saved = toml::to_string_pretty(&app.demo).unwrap();
        let reloaded = Demo::parse(&saved).unwrap();
        assert_eq!(reloaded.layout, global);
        assert_eq!(
            reloaded.queues[0].layout,
            Some(Layout::Preset(LayoutPreset::Rows))
        );
        app.apply(reloaded).unwrap();
        assert!(app.layout_cue.is_none());
    }
    #[test]
    fn inactive_pane_titles_use_high_contrast_theme_foregrounds() {
        use crate::config::Scheme;
        let mut app = app();
        app.cues.focused = true;
        app.resize(Rect::new(0, 0, 120, 40)).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let luminance = |color: Color| {
            let Color::Rgb(r, g, b) = color else {
                panic!("expected RGB theme")
            };
            let linear = |c: u8| {
                let c = f64::from(c) / 255.0;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        };
        for scheme in [Scheme::Ocean, Scheme::Ember, Scheme::Forest, Scheme::Mono] {
            for pane in &mut app.panes {
                pane.config.scheme = scheme;
            }
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let (fg, bg, _) = palette(scheme);
            assert!((luminance(fg) + 0.05) / (luminance(bg) + 0.05) >= 7.0);
            for rect in &app.rects {
                let title = &terminal.backend().buffer()[(rect.x + 2, rect.y)];
                assert_eq!((title.fg, title.bg), (fg, bg));
            }
        }
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
            advance_after_ms: None,
            layout: None,
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
        app.resize(Rect::new(0, 0, 160, 40)).unwrap();
        // Wait for an actual prompt, not a startup delay or the terminal's echo
        // of input queued before the shell has initialized its line editor.
        for pane in &mut app.panes {
            pane.write(b"PS1='NYSOS_READY> '\r").unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.tick().unwrap();
            if app.panes.iter().all(|pane| {
                let grid = pane.term.grid();
                let row = &grid[grid.cursor.point.line];
                (0..grid.cursor.point.column.0)
                    .map(|column| row[alacritty_terminal::index::Column(column)].c)
                    .collect::<String>()
                    == "NYSOS_READY> "
            }) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "shell prompts did not become ready"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
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
    fn dismissing_preview_restores_every_pane_cell() {
        let mut app = app();
        app.cues.focused = true;
        for (width, height) in [(120, 30), (80, 24), (40, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            app.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let baseline = terminal.backend().buffer().clone();
            for _ in 0..3 {
                key(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
                terminal.draw(|frame| app.draw(frame)).unwrap();
                assert_ne!(terminal.backend().buffer(), &baseline);
                key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
                terminal.draw(|frame| app.draw(frame)).unwrap();
                assert_eq!(terminal.backend().buffer(), &baseline);
            }
        }
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
                assert!(text.contains("No pending action for this cue"));
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
        assert!(app.cues.focused); // Arrow keys work immediately after launch.
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
    fn nested_layout_resize_keyboard_mouse_and_editor_preserve_sessions() {
        let mut demo = Demo::parse(include_str!("../examples/nested-layout.toml")).unwrap();
        for pane in &mut demo.panes {
            pane.shell = crate::config::test_shell();
            pane.args = crate::config::test_shell_args();
        }
        let mut app = App::new(demo, "demo.toml".into()).unwrap();
        key(&mut app, KeyCode::Tab, KeyModifiers::NONE); // Leave the initial cue focus.
        let area = Rect::new(0, 0, 146, 66);
        app.resize(area).unwrap();
        let original = app.rects.clone();
        let original_layout = app.demo.layout.clone();
        // Font zoom/window changes alter cell dimensions, never stored ratios.
        for (w, h) in [(106, 46), (186, 86), (8, 4), (1, 1)] {
            app.resize(Rect::new(0, 0, w, h)).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            assert_eq!(app.demo.layout, original_layout);
        }
        app.resize(area).unwrap();
        assert_eq!(app.rects, original);
        // Width grows through the outer column; height through its nested row.
        for (key_code, dimension) in [('>', 0), ('+', 1)] {
            let before = app.rects[0];
            key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
            key(&mut app, KeyCode::Char(key_code), KeyModifiers::NONE);
            app.resize(area).unwrap();
            if dimension == 0 {
                assert!(app.rects[0].width > before.width);
            } else {
                assert!(app.rects[0].height > before.height);
            }
        }
        let before = app.rects.clone();
        let edge = before[1].right() - 1;
        let row = before[1].y + 3;
        for (kind, column) in [
            (MouseEventKind::Down(MouseButton::Left), edge),
            (MouseEventKind::Drag(MouseButton::Left), edge + 5),
        ] {
            app.mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            })
            .unwrap();
        }
        app.resize(area).unwrap();
        assert!(app.rects[1].width > before[1].width);
        assert_eq!(app.rects[0], before[0]);
        assert_eq!(app.rects[3], before[3]);
        // A terminal resize cancels dragging, so old coordinates cannot resize a new grid.
        assert!(app.drag.is_some());
        app.resize(Rect::new(0, 0, 126, 50)).unwrap();
        assert!(app.drag.is_none());
        let resized = app.demo.layout.clone();
        let mut parser: alacritty_terminal::vte::ansi::Processor =
            alacritty_terminal::vte::ansi::Processor::new();
        parser.advance(&mut app.panes[0].term, b"KEEP_SESSION");
        app.edit_demo().unwrap();
        key(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(app.demo.layout, resized);
        assert!(
            app.panes[0]
                .term
                .renderable_content()
                .display_iter
                .map(|c| c.cell.c)
                .collect::<String>()
                .contains("KEEP_SESSION")
        );
        app.add_pane().unwrap();
        app.demo.validate().unwrap();
        app.resize(area).unwrap();
        assert!(app.rects.last().unwrap().width > 2);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested.toml");
        app.demo.save(&path).unwrap();
        assert_eq!(Demo::load(&path).unwrap().layout, app.demo.layout);
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
