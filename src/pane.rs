// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Command, PaneConfig, Scheme};
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Grid, Scroll},
    index::{Column, Line},
    term::{
        Config, TermMode,
        cell::{Cell, Flags, Hyperlink, LineLength},
    },
    vte::{
        self,
        ansi::{self, NamedColor},
    },
};
use anyhow::{Context, Result, bail};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

const HISTORY_LIMIT: usize = 10_000;
const GUTTER_WIDTH: u16 = 8;

pub struct Listener(Sender<Event>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}
#[derive(Clone, Copy)]
struct Size {
    cols: u16,
    rows: u16,
}
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

struct Bookmark {
    line: i32,
    row: usize,
}

struct ResizeMarker {
    number: u64,
    logical_start: bool,
    cues: Vec<usize>,
}

/// Reflow an annotated copy with Alacritty's own algorithm. Hyperlink metadata
/// carries anchors without changing characters, wrap flags, or line lengths.
/// No marker is ever installed in the live terminal or sent to a PTY.
struct ResizeTracker {
    grid: Grid<Cell>,
    markers: HashMap<String, ResizeMarker>,
}
impl ResizeTracker {
    fn new(pane: &Pane) -> Self {
        let mut grid = pane.term.grid().clone();
        let mut markers = HashMap::new();
        let mut number = pane.first_number;
        let mut continuation = pane.first_continuation;
        for row in -(grid.history_size() as i32)..grid.screen_lines() as i32 {
            let line = Line(row);
            // Existing links are irrelevant to the copy and cannot masquerade
            // as anchors. Arc-backed extras use copy-on-write.
            for cell in &mut grid[line][..] {
                cell.set_hyperlink(None);
            }
            let last = grid[line].line_length().0.saturating_sub(1);
            for column in [0, last] {
                if column == 0 && markers.contains_key(&format!("{row}:0")) {
                    continue;
                }
                let id = format!("{row}:{column}");
                let cues = if column == 0 {
                    pane.bookmarks
                        .iter()
                        .filter(|(_, bookmark)| bookmark.line == row)
                        .map(|(&cue, _)| cue)
                        .collect()
                } else {
                    vec![]
                };
                markers.insert(
                    id.clone(),
                    ResizeMarker {
                        number,
                        logical_start: column == 0 && !continuation,
                        cues,
                    },
                );
                grid[line][Column(column)]
                    .set_hyperlink(Some(Hyperlink::new(Some(id), "nysos-resize-anchor".into())));
            }
            continuation = grid[line][Column(grid.columns() - 1)]
                .flags
                .contains(Flags::WRAPLINE);
            if !continuation {
                number = number.saturating_add(1);
            }
        }
        Self { grid, markers }
    }
    fn resize(&mut self, size: Size) {
        self.grid
            .resize(true, size.rows as usize, size.cols as usize);
    }
    fn anchors(&self) -> (HashMap<usize, i32>, Option<(u64, bool)>) {
        let mut cues = HashMap::new();
        let mut numbering = None;
        let mut logical_offset = 0;
        let oldest = -(self.grid.history_size() as i32);
        let starts_at_head = self.grid[Line(oldest)][Column(0)]
            .hyperlink()
            .and_then(|link| self.markers.get(link.id()))
            .is_some_and(|marker| marker.logical_start);
        for row in oldest..self.grid.screen_lines() as i32 {
            for cell in &self.grid[Line(row)][..] {
                let Some(link) = cell.hyperlink() else {
                    continue;
                };
                let Some(marker) = self.markers.get(link.id()) else {
                    continue;
                };
                numbering.get_or_insert((
                    marker.number.saturating_sub(logical_offset).max(1),
                    !starts_at_head,
                ));
                for &cue in &marker.cues {
                    cues.insert(cue, row);
                }
            }
            if !self.grid[Line(row)][Column(self.grid.columns() - 1)]
                .flags
                .contains(Flags::WRAPLINE)
            {
                logical_offset += 1;
            }
        }
        (cues, numbering)
    }
}

#[derive(Default)]
struct ScreenSwitch(Option<bool>);
impl vte::Perform for ScreenSwitch {
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if !ignore
            && intermediates == b"?"
            && matches!(action, 'h' | 'l')
            && params
                .iter()
                .any(|param| matches!(param.first(), Some(47 | 1047 | 1049)))
        {
            self.0 = Some(action == 'h');
        }
    }
}

pub struct Pane {
    pub config: PaneConfig,
    pub term: Term<Listener>,
    parser: ansi::Processor,
    screen_parser: vte::Parser,
    inactive_numbering: Option<ResizeTracker>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Option<Box<dyn Child + Send + Sync>>,
    output: Receiver<Vec<u8>>,
    events: Receiver<Event>,
    size: Size,
    pub exited: bool,
    number_tracking: bool,
    show_line_numbers: bool,
    first_number: u64,
    first_continuation: bool,
    number_head: Vec<bool>,
    bookmarks: std::collections::HashMap<usize, Bookmark>,
}
impl Pane {
    pub fn spawn(config: PaneConfig) -> Result<Self> {
        let size = Size { cols: 80, rows: 24 };
        let pair = native_pty_system().openpty(PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = CommandBuilder::new(&config.shell);
        command.args(&config.args);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        // portable-pty otherwise defaults to HOME (which can be nonexistent in
        // isolated builds), rather than inheriting the launch directory.
        let cwd = match &config.cwd {
            Some(cwd) => std::path::PathBuf::from(cwd),
            None => std::env::current_dir().context("Reading launch directory")?,
        };
        if !cwd.is_dir() {
            bail!(
                "Working directory for pane '{}' does not exist or is not a directory: {}",
                config.id,
                cwd.display()
            );
        }
        command.cwd(cwd);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let child = pair
            .slave
            .spawn_command(command)
            .with_context(|| format!("Starting {} in {}", config.shell, config.id))?;
        drop(pair.slave);
        let (tx, output) = mpsc::sync_channel(64);
        thread::spawn(move || {
            let mut bytes = [0; 8192];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(bytes[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let (tx, events) = mpsc::channel();
        Ok(Self {
            config,
            term: Term::new(
                Config {
                    scrolling_history: HISTORY_LIMIT,
                    ..Config::default()
                },
                &size,
                Listener(tx),
            ),
            parser: ansi::Processor::new(),
            screen_parser: vte::Parser::new(),
            inactive_numbering: None,
            master: pair.master,
            writer,
            child: Some(child),
            output,
            events,
            size,
            exited: false,
            number_tracking: false,
            show_line_numbers: false,
            first_number: 1,
            first_continuation: false,
            number_head: vec![],
            bookmarks: Default::default(),
        })
    }
    pub fn pump(&mut self) -> Result<()> {
        // Bound work per frame so a noisy process cannot starve UI input.
        for _ in 0..64 {
            let Ok(bytes) = self.output.try_recv() else {
                break;
            };
            self.process_output(&bytes);
        }
        while let Ok(event) = self.events.try_recv() {
            let reply = match event {
                Event::PtyWrite(text) => Some(text),
                Event::TextAreaSizeRequest(format) => {
                    Some(format(alacritty_terminal::event::WindowSize {
                        num_lines: self.size.rows,
                        num_cols: self.size.cols,
                        cell_width: 0,
                        cell_height: 0,
                    }))
                }
                Event::ColorRequest(index, format) => {
                    let color = indexed_rgb(index, self.config.scheme);
                    Some(format(ansi::Rgb {
                        r: color.0,
                        g: color.1,
                        b: color.2,
                    }))
                }
                _ => None,
            };
            if let Some(reply) = reply {
                let _ = self.writer.write_all(reply.as_bytes());
            }
        }
        self.exited = self
            .child
            .as_mut()
            .expect("live child handle")
            .try_wait()?
            .is_some();
        Ok(())
    }
    fn process_output(&mut self, bytes: &[u8]) {
        if self.bookmarks.is_empty() && !self.number_tracking {
            self.screen_parser
                .advance(&mut ScreenSwitch::default(), bytes);
            self.parser.advance(&mut self.term, bytes);
            return;
        }
        // Observe each terminal operation separately, including history deletion
        // followed by fresh output in the same PTY read.
        for &byte in bytes {
            self.prepare_number_head();
            let before = self.history_state();
            let mut switch = ScreenSwitch::default();
            self.screen_parser.advance(&mut switch, &[byte]);
            let primary = (switch.0 == Some(true) && !before.2 && self.number_tracking)
                .then(|| ResizeTracker::new(self));
            self.parser.advance(&mut self.term, &[byte]);
            self.track_history(before);
            let alternate = self.term.mode().contains(TermMode::ALT_SCREEN);
            if !before.2 && alternate {
                self.inactive_numbering = primary;
            }
            if before.2 && !alternate {
                if switch.0 == Some(false) {
                    if let Some(tracker) = self.inactive_numbering.take()
                        && let Some((number, continuation)) = tracker.anchors().1
                    {
                        self.first_number = number;
                        self.first_continuation = continuation;
                    }
                } else {
                    self.inactive_numbering = None;
                    self.reset_numbers(); // A terminal reset also discards primary history.
                }
            }
        }
    }
    fn history_state(&self) -> (usize, usize, bool) {
        (
            self.term.grid().history_size(),
            self.row_identity(0),
            self.term.mode().contains(TermMode::ALT_SCREEN),
        )
    }
    fn row_identity(&self, line: i32) -> usize {
        // Rows own separate cell allocations. Rotation moves rows without moving
        // those allocations; resizing invalidates bookmarks separately. This
        // address is compared only and never dereferenced.
        &self.term.grid()[Line(line)][Column(0)] as *const _ as usize
    }
    fn track_history(&mut self, (history, top, alternate): (usize, usize, bool)) {
        let retained = self.term.grid().history_size();
        if alternate && self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.bookmarks.clear();
            return;
        }
        if retained < history || alternate != self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.bookmarks.clear();
            self.number_head.clear();
            if !alternate && !self.term.mode().contains(TermMode::ALT_SCREEN) {
                self.reset_numbers();
            }
            return;
        }
        let delta = if top == self.row_identity(0) {
            0
        } else {
            // Even when history is full, the former top row moves into history.
            // One terminal scroll operation moves at most one screenful of rows.
            let limit = retained.min(self.term.grid().screen_lines());
            let Some(delta) = (1..=limit).find(|&n| self.row_identity(-(n as i32)) == top) else {
                self.bookmarks.clear(); // Reset or unsupported buffer rearrangement.
                self.reset_numbers();
                return;
            };
            delta as i32
        };
        let evicted = (history + delta as usize).saturating_sub(retained);
        if self.number_tracking && !alternate && evicted > 0 {
            for &wrap in self.number_head.iter().take(evicted) {
                if !wrap {
                    self.first_number = self.first_number.saturating_add(1);
                }
                self.first_continuation = wrap;
            }
            self.number_head.clear();
        }
        let grid = self.term.grid();
        self.bookmarks.retain(|_, bookmark| {
            let shifted = bookmark.line - delta;
            if shifted < -(retained as i32) {
                return false;
            }
            let identity = |line: i32| &grid[Line(line)][Column(0)] as *const _ as usize;
            if identity(shifted) == bookmark.row {
                bookmark.line = shifted;
                true
            } else {
                // A fixed row below a scrolling region may not have moved.
                // Other rearrangements expire the anchor instead of guessing.
                identity(bookmark.line) == bookmark.row
            }
        });
    }
    fn reset_numbers(&mut self) {
        self.first_number = 1;
        self.first_continuation = false;
        self.number_head.clear();
    }
    fn prepare_number_head(&mut self) {
        let grid = self.term.grid();
        if self.number_tracking
            && !self.term.mode().contains(TermMode::ALT_SCREEN)
            && self.number_head.is_empty()
            && grid.history_size() + grid.screen_lines() >= HISTORY_LIMIT
        {
            self.number_head = (0..grid.history_size().min(grid.screen_lines()))
                .map(|n| {
                    grid[Line(-(grid.history_size() as i32) + n as i32)][Column(grid.columns() - 1)]
                        .flags
                        .contains(Flags::WRAPLINE)
                })
                .collect();
        }
    }
    pub fn configure_line_numbers(&mut self, gate: bool, global: bool) {
        if gate != self.number_tracking {
            self.reset_numbers();
            self.inactive_numbering = None;
        }
        self.number_tracking = gate;
        self.show_line_numbers = gate && self.config.line_numbers.unwrap_or(global);
    }
    /// Geometry shared by PTY sizing, rendering, cursor placement, and hit testing.
    pub fn body_area(&self, inner: Rect) -> Rect {
        let gutter = if self.show_line_numbers && inner.width >= GUTTER_WIDTH + 8 {
            GUTTER_WIDTH
        } else {
            0
        };
        Rect::new(
            inner.x + gutter,
            inner.y,
            inner.width - gutter,
            inner.height,
        )
    }
    fn render_gutter(&self, inner: Rect, body: Rect, buf: &mut Buffer) {
        if body.x == inner.x {
            return;
        }
        let (fg, bg, _) = palette(self.config.scheme);
        for y in inner.y..inner.bottom() {
            buf.set_string(inner.x, y, "        ", Style::default().fg(fg).bg(bg));
        }
        // Keep the reserved width stable when an application owns the screen.
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let grid = self.term.grid();
        let top = -(grid.display_offset() as i32);
        let bottom = (top + inner.height as i32).min(grid.screen_lines() as i32);
        let mut number = self.first_number;
        let mut continuation = self.first_continuation;
        for line in -(grid.history_size() as i32)..bottom {
            if line >= top {
                let label = if continuation {
                    "     ↳".into()
                } else if number > 999_999 {
                    "++++++".into()
                } else {
                    format!("{number:>6}")
                };
                buf.set_string(
                    inner.x,
                    inner.y + (line - top) as u16,
                    format!("{label} │"),
                    Style::default().fg(fg).bg(bg),
                );
            }
            continuation = grid[Line(line)][Column(grid.columns() - 1)]
                .flags
                .contains(Flags::WRAPLINE);
            if !continuation {
                number = number.saturating_add(1);
            }
        }
    }
    pub fn resize(&mut self, area: Rect) -> Result<()> {
        let size = Size {
            cols: area.width.max(2),
            rows: area.height.max(1),
        };
        if (size.cols, size.rows) != (self.size.cols, self.size.rows) {
            self.master.resize(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            let normal = !self.term.mode().contains(TermMode::ALT_SCREEN);
            let mut tracker = (normal && (self.number_tracking || !self.bookmarks.is_empty()))
                .then(|| ResizeTracker::new(self));
            if let Some(tracker) = &mut tracker {
                tracker.resize(size);
            }
            if let Some(primary) = &mut self.inactive_numbering {
                primary.resize(size);
            }
            self.term.resize(size);
            self.number_head.clear();
            if let Some(tracker) = tracker {
                let (positions, numbering) = tracker.anchors();
                self.bookmarks = positions
                    .into_iter()
                    .map(|(cue, line)| {
                        (
                            cue,
                            Bookmark {
                                line,
                                row: self.row_identity(line),
                            },
                        )
                    })
                    .collect();
                if let Some((number, continuation)) = numbering {
                    self.first_number = number;
                    self.first_continuation = continuation;
                }
            }
            self.size = size;
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if self.exited {
            bail!(
                "Pane '{}' has exited; restart with prefix x",
                self.config.id
            );
        }
        self.term.scroll_display(Scroll::Bottom);
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }
    pub fn bookmark(&mut self, cue: usize) {
        self.bookmarks.remove(&cue);
        if !self.term.mode().contains(TermMode::ALT_SCREEN) {
            let line = self.term.grid().cursor.point.line.0;
            self.bookmarks.insert(
                cue,
                Bookmark {
                    line,
                    row: self.row_identity(line),
                },
            );
        }
    }
    pub fn forget_bookmarks(&mut self) {
        self.bookmarks.clear();
    }
    pub fn restore_bookmark(&mut self, cue: usize) -> bool {
        let Some(bookmark) = self.bookmarks.get(&cue) else {
            return false;
        };
        let offset = (-bookmark.line).max(0);
        self.term.scroll_display(Scroll::Bottom);
        self.term.scroll_display(Scroll::Delta(offset));
        true
    }
    pub fn send_action(&mut self, action: &Command, type_only: bool) -> Result<()> {
        if action.clear {
            // Clear the emulator, preserving history and the application's cursor.
            // Do not inject shell input or interrupt a foreground application.
            use ansi::Handler;
            self.prepare_number_head();
            let before = self.history_state();
            self.term.clear_screen(ansi::ClearMode::All);
            self.track_history(before);
            self.term.scroll_display(Scroll::Bottom);
            Ok(())
        } else if !action.keys.is_empty() {
            let mut bytes = Vec::new();
            for key in &action.keys {
                let encoded =
                    crate::input::key_bytes(crate::input::parse_key(&key.key)?, *self.term.mode());
                for _ in 0..key.repeat {
                    bytes.extend_from_slice(&encoded);
                }
            }
            self.write(&bytes)
        } else if type_only {
            self.paste(&action.command)
        } else {
            self.write(format!("{}\r", action.command).as_bytes())
        }
    }
    pub fn paste(&mut self, text: &str) -> Result<()> {
        // Strip escape characters to avoid terminating bracketed paste early.
        let text = text.replace('\x1b', "");
        if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
            self.write(format!("\x1b[200~{text}\x1b[201~").as_bytes())
        } else {
            self.write(text.as_bytes())
        }
    }
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let body = self.body_area(area);
        self.render_gutter(area, body, buf);
        let area = body;
        let content = self.term.renderable_content();
        let (fg, bg, _) = palette(self.config.scheme);
        buf.set_style(area, Style::default().fg(fg).bg(bg));
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + content.display_offset as i32;
            let col = indexed.point.column.0 as u16;
            if row < 0 || row >= area.height as i32 || col >= area.width {
                continue;
            }
            let cell = indexed.cell;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let mut foreground = convert_color(cell.fg, self.config.scheme);
            let mut background = convert_color(cell.bg, self.config.scheme);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }
            let mut style = Style::default().fg(foreground).bg(background);
            for (flag, modifier) in [
                (Flags::BOLD, Modifier::BOLD),
                (Flags::ITALIC, Modifier::ITALIC),
                (Flags::UNDERLINE, Modifier::UNDERLINED),
                (Flags::DIM, Modifier::DIM),
                (Flags::STRIKEOUT, Modifier::CROSSED_OUT),
                (Flags::HIDDEN, Modifier::HIDDEN),
            ] {
                if cell.flags.contains(flag) {
                    style = style.add_modifier(modifier);
                }
            }
            // Alacritty retains tabs in its grid for text selection. Their
            // spacing is already represented by subsequent grid cells. Emitting
            // a raw tab would move the host cursor using its own tab stops,
            // corrupting Ratatui's cell positioning and incremental redraws.
            let mut symbol = if cell.c.is_control() { ' ' } else { cell.c }.to_string();
            if let Some(extra) = cell.zerowidth() {
                symbol.extend(extra);
            }
            buf[(area.x + col, area.y + row as u16)]
                .set_symbol(&symbol)
                .set_style(style);
        }
    }
    pub fn cursor(&self, area: Rect) -> Option<(u16, u16)> {
        let area = self.body_area(area);
        let content = self.term.renderable_content();
        let point = content.cursor.point;
        if content.display_offset != 0
            || !content.mode.contains(TermMode::SHOW_CURSOR)
            || point.line.0 < 0
            || point.line.0 >= area.height as i32
            || point.column.0 >= area.width as usize
        {
            return None;
        }
        Some((area.x + point.column.0 as u16, area.y + point.line.0 as u16))
    }
    pub fn url_at(&self, x: u16, y: u16) -> Option<String> {
        let content = self.term.renderable_content();
        let mut line = vec![' '; self.size.cols as usize];
        for indexed in content.display_iter {
            if indexed.point.line.0 + content.display_offset as i32 != y as i32 {
                continue;
            }
            let col = indexed.point.column.0;
            if col == x as usize
                && let Some(link) = indexed.cell.hyperlink()
            {
                return web_url(link.uri());
            }
            if col < line.len() {
                line[col] = indexed.cell.c;
            }
        }
        url_in_line(&line, x as usize)
    }
}
impl Drop for Pane {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            // Reap outside the UI thread so shutdown cannot freeze rendering.
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
pub fn web_url(text: &str) -> Option<String> {
    let parsed = url::Url::parse(text).ok()?;
    (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some())
        .then(|| parsed.to_string())
}
fn url_in_line(line: &[char], x: usize) -> Option<String> {
    if x >= line.len() || line[x].is_whitespace() {
        return None;
    }
    let start = (0..x)
        .rev()
        .find(|&i| line[i].is_whitespace())
        .map_or(0, |i| i + 1);
    let end = (x..line.len())
        .find(|&i| line[i].is_whitespace())
        .unwrap_or(line.len());
    let token: String = line[start..end].iter().collect();
    web_url(token.trim_matches([
        '"', '\'', '<', '>', '(', ')', '[', ']', '{', '}', '.', ',', ';',
    ]))
}
pub fn palette(scheme: Scheme) -> (Color, Color, Color) {
    match scheme {
        Scheme::Ocean => (
            Color::Rgb(220, 235, 245),
            Color::Rgb(14, 24, 38),
            Color::Rgb(92, 200, 240),
        ),
        Scheme::Ember => (
            Color::Rgb(245, 229, 218),
            Color::Rgb(35, 23, 24),
            Color::Rgb(245, 161, 100),
        ),
        Scheme::Forest => (
            Color::Rgb(223, 240, 223),
            Color::Rgb(17, 30, 24),
            Color::Rgb(126, 210, 154),
        ),
        Scheme::Mono => (
            Color::Rgb(230, 230, 230),
            Color::Rgb(20, 20, 20),
            Color::White,
        ),
    }
}
fn indexed_rgb(index: usize, scheme: Scheme) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (30, 30, 30),
        (205, 70, 70),
        (100, 190, 110),
        (220, 190, 90),
        (90, 145, 220),
        (185, 115, 205),
        (90, 190, 195),
        (220, 220, 220),
        (105, 105, 105),
        (255, 110, 110),
        (150, 225, 140),
        (255, 225, 135),
        (140, 180, 255),
        (220, 160, 245),
        (140, 225, 230),
        (255, 255, 255),
    ];
    match index {
        0..=15 => ANSI[index],
        16..=231 => {
            let n = index - 16;
            let level = |v: usize| if v == 0 { 0 } else { (55 + v * 40) as u8 };
            (level(n / 36), level(n / 6 % 6), level(n % 6))
        }
        232..=255 => {
            let v = (8 + (index - 232) * 10) as u8;
            (v, v, v)
        }
        _ => {
            let (fg, bg, _) = palette(scheme);
            let Color::Rgb(r, g, b) = (if index == NamedColor::Background as usize {
                bg
            } else {
                fg
            }) else {
                return (230, 230, 230);
            };
            (r, g, b)
        }
    }
}
fn convert_color(color: ansi::Color, scheme: Scheme) -> Color {
    let (r, g, b) = match color {
        ansi::Color::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        ansi::Color::Indexed(i) => indexed_rgb(i as usize, scheme),
        ansi::Color::Named(n) => indexed_rgb(n as usize, scheme),
    };
    Color::Rgb(r, g, b)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    fn numbered_pane() -> Pane {
        Pane::spawn(PaneConfig {
            shell: crate::config::test_shell(),
            args: crate::config::test_shell_args(),
            ..Default::default()
        })
        .unwrap()
    }
    fn gutter_rows(pane: &Pane, area: Rect) -> Vec<String> {
        let mut buffer = Buffer::empty(area);
        pane.render(area, &mut buffer);
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.x + GUTTER_WIDTH)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }
    #[test]
    fn gated_gutter_wraps_reflows_and_shares_cursor_and_scrollback() {
        let mut pane = numbered_pane();
        let area = Rect::new(3, 4, 24, 4);
        pane.configure_line_numbers(false, true);
        assert_eq!(pane.body_area(area), area);
        pane.configure_line_numbers(true, true);
        assert_eq!(pane.body_area(area), Rect::new(11, 4, 16, 4));
        pane.resize(pane.body_area(area)).unwrap();
        pane.process_output(b"12345678901234567\r\nnext");
        assert_eq!(
            &gutter_rows(&pane, area)[..3],
            ["     1 │", "     ↳ │", "     2 │"]
        );
        assert_eq!(pane.cursor(area), Some((15, 6)));
        let wide = Rect::new(3, 4, 32, 4);
        pane.resize(pane.body_area(wide)).unwrap();
        assert_eq!(&gutter_rows(&pane, wide)[..2], ["     1 │", "     2 │"]);
        pane.bookmark(1);
        pane.process_output(b"\r\nthree\r\nfour\r\nfive\r\nsix\r\n");
        assert!(pane.restore_bookmark(1));
        assert_eq!(gutter_rows(&pane, wide)[0], "     2 │");
        pane.term.scroll_display(Scroll::Top);
        assert_eq!(gutter_rows(&pane, wide)[0], "     1 │");
        pane.term.scroll_display(Scroll::Bottom);
        assert!(gutter_rows(&pane, wide)[0].contains('4'));
        pane.process_output(b"\x1b[?1049hALT\r\n");
        assert!(gutter_rows(&pane, wide).iter().all(|row| row == "        "));
        pane.process_output(b"\x1b[?1049l");
        pane.term.scroll_display(Scroll::Top);
        assert_eq!(gutter_rows(&pane, wide)[0], "     1 │");
        pane.config.line_numbers = Some(false);
        pane.configure_line_numbers(true, true);
        assert_eq!(pane.body_area(area), area);
        pane.config.line_numbers = Some(true);
        pane.configure_line_numbers(true, true);
        let tiny = Rect::new(0, 0, 12, 3);
        assert_eq!(pane.body_area(tiny), tiny);
    }
    #[test]
    fn bookmarks_and_line_labels_survive_repeated_reflow_at_history_capacity() {
        let mut pane = numbered_pane();
        pane.configure_line_numbers(true, true);
        let mut area = Rect::new(0, 0, 48, 8);
        pane.resize(pane.body_area(area)).unwrap();
        pane.process_output(&b"history line\r\n".repeat(HISTORY_LIMIT + 20));
        pane.bookmark(42);
        pane.process_output("\x1b]8;;https://example.com/anchor\x1b\\ANCHOR 界🙂 original output with a long wrapped command\x1b]8;;\x1b\\\r\n".as_bytes());
        pane.process_output(&b"later output\r\n".repeat(40));
        assert!(pane.restore_bookmark(42));
        let label = gutter_rows(&pane, area)[0].clone();
        assert_ne!(label, "     1 │");
        for (width, height) in [(24, 6), (80, 12), (18, 4), (48, 8)] {
            area = Rect::new(0, 0, width, height);
            pane.resize(pane.body_area(area)).unwrap();
            assert!(pane.restore_bookmark(42), "lost anchor at {width}x{height}");
            assert_eq!(gutter_rows(&pane, area)[0], label);
            assert_eq!(
                pane.url_at(0, 0).as_deref(),
                Some("https://example.com/anchor")
            );
            let mut buffer = Buffer::empty(area);
            pane.render(area, &mut buffer);
            assert_eq!(buffer[(8, 0)].symbol(), "A");
        }
        // Resizing an alternate application must also reflow the hidden primary
        // numbering without replacing its original labels or hyperlink content.
        pane.process_output(b"\x1b[?1049h");
        area.width = 20;
        pane.resize(pane.body_area(area)).unwrap();
        pane.process_output(b"\x1b[?1049l");
        // Bookmarks expire on alternate-screen transitions; numbering does not.
        assert!(!pane.restore_bookmark(42));
        let grid = pane.term.grid();
        let anchor = (-(grid.history_size() as i32)..grid.screen_lines() as i32)
            .find(|&line| grid[Line(line)][Column(0)].c == 'A')
            .unwrap();
        pane.term.scroll_display(Scroll::Bottom);
        pane.term.scroll_display(Scroll::Delta(-anchor));
        assert_eq!(gutter_rows(&pane, area)[0], label);
    }
    #[test]
    fn gutter_numbers_survive_history_eviction_and_native_clear() {
        let mut pane = numbered_pane();
        pane.configure_line_numbers(true, true);
        let area = Rect::new(0, 0, 28, 4);
        pane.resize(pane.body_area(area)).unwrap();
        pane.process_output(&b"line\r\n".repeat(HISTORY_LIMIT + 12));
        assert_eq!(pane.first_number, 10);
        pane.term.scroll_display(Scroll::Top);
        assert_eq!(gutter_rows(&pane, area)[0], "    10 │");
        pane.send_action(
            &Command {
                clear: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert!(pane.first_number >= 10);
        pane.process_output(b"\x1b[?1049h");
        let first = pane.first_number;
        pane.process_output(&b"alt\r\n".repeat(40));
        pane.process_output(b"\x1b[?1049l");
        assert_eq!(pane.first_number, first);
        pane.process_output(b"\x1b[?1049h");
        pane.resize(Rect::new(0, 0, 24, 4)).unwrap();
        pane.process_output(b"\x1b[?1049l");
        assert_eq!(pane.first_number, first);
        pane.process_output(b"\x1b[3J");
        assert_eq!(pane.first_number, 1);
    }
    #[test]
    fn key_actions_repeat_native_clear_and_history_bookmarks() {
        struct Capture(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut pane = Pane::spawn(PaneConfig {
            shell: crate::config::test_shell(),
            args: crate::config::test_shell_args(),
            ..Default::default()
        })
        .unwrap();
        let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        pane.writer = Box::new(Capture(bytes.clone()));
        let action = Command {
            keys: vec![
                crate::config::KeyPress {
                    key: "Ctrl+C".into(),
                    repeat: 2,
                },
                crate::config::KeyPress {
                    key: "Ctrl+D".into(),
                    repeat: 1,
                },
                crate::config::KeyPress {
                    key: "<esc>".into(),
                    repeat: 1,
                },
                crate::config::KeyPress {
                    key: "Left".into(),
                    repeat: 3,
                },
            ],
            ..Default::default()
        };
        for type_only in [false, true] {
            pane.send_action(&action, type_only).unwrap();
        }
        let expected = b"\x03\x03\x04\x1b\x1b[D\x1b[D\x1b[D".repeat(2);
        assert_eq!(*bytes.lock().unwrap(), expected);
        pane.resize(Rect::new(0, 0, 40, 4)).unwrap();
        pane.process_output(b"prompt> ");
        pane.bookmark(7);
        pane.process_output(b"command\r\n1\r\n2\r\n3\r\n4\r\n5\r\n");
        assert!(pane.restore_bookmark(7));
        assert_eq!(
            pane.term.grid().display_offset(),
            pane.term.grid().history_size()
        );
        assert!(
            pane.term
                .renderable_content()
                .display_iter
                .any(|c| c.cell.c == 'p')
        );
        pane.send_action(
            &Command {
                clear: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert_eq!(*bytes.lock().unwrap(), expected); // Native clear sent no PTY input.
        assert!(
            pane.term
                .renderable_content()
                .display_iter
                .all(|c| c.cell.c == ' ')
        );
        assert!(pane.restore_bookmark(7));
        pane.resize(Rect::new(0, 0, 20, 4)).unwrap();
        assert!(pane.restore_bookmark(7));
        pane.bookmark(8);
        pane.process_output(b"\x1b[3J");
        assert!(!pane.restore_bookmark(8));
        pane.bookmark(9);
        pane.process_output(&b"line\r\n".repeat(HISTORY_LIMIT + 8));
        assert!(!pane.restore_bookmark(9)); // The bookmarked line was evicted.
        assert_eq!(pane.term.grid().history_size(), HISTORY_LIMIT);
        pane.bookmark(10);
        pane.process_output(&b"more\r\n".repeat(12));
        assert!(pane.restore_bookmark(10)); // New bookmarks still work at capacity.
        assert!(pane.term.grid().display_offset() >= 9);
        pane.bookmark(11);
        pane.process_output(b"\x1b[2;4r\x1b[4;1H\n");
        assert!(!pane.restore_bookmark(11)); // Partial-region rearrangement, not history.
    }
    #[test]
    fn invalid_working_directory_is_not_silently_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let result = Pane::spawn(PaneConfig {
            shell: crate::config::test_shell(),
            args: crate::config::test_shell_args(),
            cwd: Some(dir.path().join("missing").to_string_lossy().into_owned()),
            ..Default::default()
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("Working directory")
        );
    }
    #[cfg(unix)]
    #[test]
    fn real_pty_executes_and_receives_resize() {
        use std::time::{Duration, Instant};
        let mut pane = Pane::spawn(PaneConfig {
            shell: crate::config::test_shell(),
            args: crate::config::test_shell_args(),
            ..Default::default()
        })
        .unwrap();
        pane.resize(Rect::new(0, 0, 73, 19)).unwrap();
        // Split the sentinel so the command echo cannot satisfy the assertion.
        pane.write(b"printf 'NY%sOS_OK\\n' S; stty size\r").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            pane.pump().unwrap();
            let screen: String = pane
                .term
                .renderable_content()
                .display_iter
                .map(|c| c.cell.c)
                .collect();
            if screen.contains("NYSOS_OK") && screen.contains("19 73") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "PTY did not produce expected output: {screen}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn urls_are_web_only_and_use_cell_offsets() {
        let chars: Vec<_> = "界 https://example.com/demo, end".chars().collect();
        assert_eq!(
            url_in_line(&chars, 10).as_deref(),
            Some("https://example.com/demo")
        );
        assert!(web_url("file:///etc/passwd").is_none());
        assert!(web_url("javascript:alert(1)").is_none());
    }
    #[cfg(unix)]
    #[test]
    fn tabbed_output_and_overlay_restoration_match_host_screen() {
        use ratatui::{Terminal, backend::CrosstermBackend};
        // Exercise ANSI colors even in CI/agent environments with NO_COLOR set.
        crossterm::style::force_color_output(true);
        let mut pane = Pane::spawn(PaneConfig {
            shell: crate::config::test_shell(),
            args: crate::config::test_shell_args(),
            ..Default::default()
        })
        .unwrap();
        let area = Rect::new(31, 5, 33, 6);
        pane.resize(area).unwrap();
        // Like columnar ls output in the demo, followed by repeated final cues.
        pane.parser
            .advance(&mut pane.term, b"first\tsecond\tthird\r\n");
        let (tx, _) = mpsc::channel();
        let mut outer = Term::new(
            Config::default(),
            &Size {
                cols: 100,
                rows: 30,
            },
            Listener(tx),
        );
        let mut parser: ansi::Processor = ansi::Processor::new();
        #[derive(Clone, Default)]
        struct Capture(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.borrow_mut().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let capture = Capture::default();
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(capture.clone()),
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(Rect::new(0, 0, 100, 30)),
            },
        )
        .unwrap();
        for frame_index in 0..24 {
            let expected_buffer = terminal
                .draw(|frame| {
                    frame.render_widget(
                        ratatui::widgets::Block::bordered(),
                        Rect::new(30, 4, 35, 8),
                    );
                    pane.render(area, frame.buffer_mut());
                    if frame_index % 2 == 1 {
                        let popup = Rect::new(33, 6, 27, 3);
                        frame.render_widget(ratatui::widgets::Clear, popup);
                        frame.render_widget(
                            ratatui::widgets::Paragraph::new("Preview command")
                                .block(ratatui::widgets::Block::bordered())
                                .style(
                                    Style::default().fg(Color::White).bg(Color::Rgb(28, 35, 48)),
                                ),
                            popup,
                        );
                    }
                })
                .unwrap()
                .buffer
                .clone();
            let bytes = std::mem::take(&mut *capture.0.borrow_mut());

            parser.advance(&mut outer, &bytes);
            // Check the actual emitted ANSI stream, not just the intended buffer.
            for cell in outer.renderable_content().display_iter {
                let x = cell.point.column.0 as u16;
                let y = cell.point.line.0 as u16;
                let expected = &expected_buffer[(x, y)];
                assert_eq!(
                    cell.cell.c,
                    expected.symbol().chars().next().unwrap(),
                    "host cell at {x},{y}"
                );
                if let Color::Rgb(r, g, b) = expected.bg {
                    assert_eq!(
                        cell.cell.bg,
                        ansi::Color::Spec(ansi::Rgb { r, g, b }),
                        "host background at {x},{y} in frame {frame_index}"
                    );
                }
            }
            pane.parser
                .advance(&mut pane.term, b"Finished demonstration\r\n");
        }
    }
    #[test]
    fn ansi_parser_preserves_color_and_cursor() {
        let (tx, _) = mpsc::channel();
        let mut term = Term::new(Config::default(), &Size { cols: 20, rows: 4 }, Listener(tx));
        let mut parser: ansi::Processor = ansi::Processor::new();
        parser.advance(&mut term, b"\x1b[31mhello\r\nworld");
        let mut content = term.renderable_content();
        assert_eq!(content.cursor.point.line.0, 1);
        assert_eq!(content.cursor.point.column.0, 5);
        assert!(
            content
                .display_iter
                .any(|c| c.cell.c == 'h' && c.cell.fg == ansi::Color::Named(NamedColor::Red))
        );
    }
}
