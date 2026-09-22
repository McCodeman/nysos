mod app;
mod cli;
mod config;
mod cues;
mod input;
mod layout;
mod pane;
mod platform;
mod prefix;
mod shell_setup;

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};

use cli::Args;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}
fn main() -> Result<()> {
    let args = Args::parse();
    if args.add_to_path.is_some() || args.install_completions.is_some() {
        return shell_setup::install(&args);
    }
    if let Some(path) = args.init {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .context("Creating starter demo (destination must not exist)")?;
        file.write_all(toml::to_string_pretty(&config::Demo::builtin()?)?.as_bytes())?;
        println!("Created {}", path.display());
        return Ok(());
    }
    let mut demo = if let Some(path) = &args.config {
        config::Demo::load(path)?
    } else if args.demo {
        config::Demo::builtin()?
    } else {
        config::Demo::default()
    };
    if let Some(prefix) = args.prefix {
        demo.prefix = prefix;
    }
    if let Some(profile) = args.terminal_keys {
        demo.terminal_keys = profile;
    }
    demo.validate()?;
    if args.check {
        println!(
            "Valid: {} panes, {} queue items",
            demo.panes.len(),
            demo.queues.len()
        );
        return Ok(());
    }
    if cfg!(windows) {
        bail!("Interactive Windows support is deferred; use macOS, Linux, or WSL");
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("nysos requires an interactive terminal; use --check to validate a demo");
    }
    let mut app = app::App::new(demo, args.config.unwrap_or_else(|| "demo.toml".into()))?;
    app.debug_keys = args.debug_keys;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        drop(TerminalGuard);
        previous_hook(info);
    }));
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    while !app.quit {
        app.tick()?;
        let size = terminal.size()?;
        app.resize(ratatui::layout::Rect::new(0, 0, size.width, size.height))?;
        terminal.draw(|frame| app.draw(frame))?;
        if event::poll(Duration::from_millis(16))? {
            app.event(event::read()?);
        }
    }
    Ok(())
}
