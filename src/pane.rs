use crate::config::{PaneConfig, Scheme};
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    term::{Config, TermMode, cell::Flags},
    vte::ansi::{self, NamedColor},
};
use anyhow::{Context, Result, bail};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use std::{
    io::{Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

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

pub struct Pane {
    pub config: PaneConfig,
    pub term: Term<Listener>,
    parser: ansi::Processor,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Option<Box<dyn Child + Send + Sync>>,
    output: Receiver<Vec<u8>>,
    events: Receiver<Event>,
    size: Size,
    pub exited: bool,
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
            term: Term::new(Config::default(), &size, Listener(tx)),
            parser: ansi::Processor::new(),
            master: pair.master,
            writer,
            child: Some(child),
            output,
            events,
            size,
            exited: false,
        })
    }
    pub fn pump(&mut self) -> Result<()> {
        // Bound work per frame so a noisy process cannot starve UI input.
        for _ in 0..64 {
            let Ok(bytes) = self.output.try_recv() else {
                break;
            };
            self.parser.advance(&mut self.term, &bytes);
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
            self.term.resize(size);
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
            let mut symbol = cell.c.to_string();
            if let Some(extra) = cell.zerowidth() {
                symbol.extend(extra);
            }
            buf[(area.x + col, area.y + row as u16)]
                .set_symbol(&symbol)
                .set_style(style);
        }
    }
    pub fn cursor(&self, area: Rect) -> Option<(u16, u16)> {
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
