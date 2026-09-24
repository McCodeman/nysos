// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ShellTarget {
    Bash,
    Zsh,
    All,
}

/// Command-line definition shared by the executable and documentation generator.
#[derive(Parser)]
#[command(
    name = "nysos",
    version,
    long_version = include!(concat!(env!("OUT_DIR"), "/version.rs")),
    about = "Multi-pane scripted and interactive terminal demonstrations",
    long_about = "Run named, interactive shell panes alongside a manual or timed cue playlist.\nConfigure global and per-cue pane layouts, sizing, colors, shells, commands, key presses, and looping in a TOML demo file.\nLoading a demo never automatically executes its queued commands.",
    after_help = "Press Ctrl-G, then ? for interactive help. Use --help for examples and details.",
    after_long_help = "EXAMPLES:\n  nysos                              Start two shells with an empty cue list\n  nysos --demo                       Load the built-in twelve-cue demo\n  nysos --demo --disable-feature line-numbers  Disable line gutters if needed\n  nysos --add-to-path                 Add this binary directory to Bash/Zsh PATH\n  nysos --install-completions         Install Bash and Zsh completions\n  nysos --init demo.toml              Write a starter file without overwriting\n  nysos --config demo.toml --check     Validate without opening shells\n  nysos --config demo.toml             Present a saved demo\n\nINTERACTIVE CONTROLS:\n  Press Ctrl-G, release, then:\n    n / Enter  Send next command       s  Skip next command\n    e          Edit current queue     o  Edit full demo: title, cues, panes\n    a          Add a live pane        Tab  Focus next pane\n    < / >      Narrow / widen pane    - / +  Shorten / heighten pane\n    0          Focus cue list         c  Show/hide cue list\n    #          Toggle pane line numbers (requires line-numbers feature gate)\n    ?          Show all controls      q  Quit and close shell sessions\n  In the cue list: Up/Down selects, Enter sends a cue, e edits it, o edits the full demo.\n  Press p in the cue list to preview each pane’s next command or key/clear action; Esc closes.\n  Press t to type commands without Enter; key/clear actions are sent immediately.\n  In Cues: s restores recorded layouts and bookmarks; b restores the live layout and bottom.\n  Space pauses/resumes an armed cue timer; browsing cancels it.\n  Cue commands are sent in order without waiting for completion.\n  Alt-Left/Right rotates focus; Ghostty mappings also accept Alt-B/F. Click to focus; drag dividers to resize.\n  Cmd-click opens URLs locally on macOS; Alt-click is the portable fallback.\n\nNOTES:\n  Default Ctrl-G avoids tmux Ctrl-B; --prefix selects another key.\n  Over SSH use ssh -t; URL openers run on the nysos host.\n  Interactive mode requires a terminal on both stdin and stdout.\n  Wait for the target shell prompt before sending the next command.\n  --check validates TOML and pane references, not shell availability or commands.\n  No demo file is auto-discovered. Paths are relative to the launch directory.\n  macOS is primary, Linux secondary; native Windows interaction is deferred.\n\nEXIT STATUS:\n  0  Success (including help/version)\n  1  Configuration, file, PTY, or runtime error\n  2  Invalid command-line arguments\n\nDOCUMENTATION:\n  See docs/ for user guides and the complete configuration reference.\n  Run 'man nysos' after installing the manpage with 'make man-install'."
)]
pub struct Args {
    /// Enable an experimental runtime feature (repeatable; also configurable in TOML)
    #[arg(long, value_enum, value_name = "FEATURE", conflicts_with_all = ["init", "add_to_path", "install_completions"])]
    pub enable_feature: Vec<crate::features::Feature>,
    /// Disable an experimental feature; takes precedence over TOML and --enable-feature
    #[arg(long, value_enum, value_name = "FEATURE", conflicts_with_all = ["init", "add_to_path", "install_completions"])]
    pub disable_feature: Vec<crate::features::Feature>,
    /// Load the built-in twelve-cue guided tour of commands, layouts, and pane controls
    #[arg(long, conflicts_with_all = ["config", "init", "add_to_path", "install_completions"])]
    pub demo: bool,
    /// Show the last key event and editor mode for shortcut troubleshooting
    #[arg(long, conflicts_with_all = ["init", "check", "add_to_path", "install_completions"])]
    pub debug_keys: bool,
    /// Disable mouse capture; use prefix then < / > for width or - / + for height
    #[arg(long, conflicts_with_all = ["init", "check", "add_to_path", "install_completions"])]
    pub no_mouse: bool,
    /// Terminal key mappings: auto detects Ghostty from TERM_PROGRAM or TERM
    #[arg(long, value_enum, value_name = "PROFILE", conflicts_with_all = ["init", "add_to_path", "install_completions"])]
    pub terminal_keys: Option<crate::prefix::TerminalKeys>,
    /// Override the demo control prefix (default: ctrl-g; avoids default tmux prefix)
    #[arg(long, value_enum, value_name = "KEY", conflicts_with_all = ["init", "add_to_path", "install_completions"])]
    pub prefix: Option<crate::prefix::Prefix>,
    /// Print full version, build date/time (UTC), Git metadata, compiler, target, and build profile
    #[arg(long, action = clap::ArgAction::Version)]
    pub version_full: Option<bool>,
    /// Load a demo TOML file
    #[arg(
        short,
        long,
        value_name = "PATH",
        long_help = "Load a demo TOML file. No file is loaded automatically, including demo.toml. Relative paths resolve from the launch directory."
    )]
    pub config: Option<PathBuf>,
    /// Validate the selected demo without opening shells
    #[arg(
        long,
        long_help = "Validate TOML syntax, pane count, IDs, titles, weights, and command targets without spawning shells. Use --demo to check the built-in demo; with neither --config nor --demo, checks the empty interactive configuration. Does not verify shell paths, working directories, or command syntax."
    )]
    pub check: bool,
    /// Write a starter TOML file and exit (never overwrite)
    #[arg(long, value_name = "PATH", conflicts_with_all = ["config", "check"], long_help = "Write the built-in twelve-cue demo using /bin/sh and exit. The destination must not exist and its parent directory must already exist. Cannot be combined with --config or --check.")]
    pub init: Option<PathBuf>,
    /// Add the binary directory to shell startup files (default: all)
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "all", value_name = "SHELL", conflicts_with_all = ["config", "check", "init"], long_help = "Add the running binary's directory to PATH for bash, zsh, or both (default: all). Updates marked blocks in user startup files, preserves other content, and backs up existing files. Use --bin-dir to select a stable installation directory. Does not copy the executable or alter the current shell.")]
    pub add_to_path: Option<ShellTarget>,
    /// Directory containing nysos to add to PATH
    #[arg(long, value_name = "DIR", requires = "add_to_path")]
    pub bin_dir: Option<PathBuf>,
    /// Install generated shell completions and enable them (default: all)
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "all", value_name = "SHELL", conflicts_with_all = ["config", "check", "init"], long_help = "Install bash, zsh, or both completion scripts (default: all) under $XDG_DATA_HOME/nysos/completions, falling back to ~/.local/share. Updates user startup files to load them; respects ZDOTDIR. Safe to repeat. Can be combined with --add-to-path. Open a new terminal afterward.")]
    pub install_completions: Option<ShellTarget>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_gate_options_validate_known_names_and_conflicts() {
        let args =
            Args::try_parse_from(["nysos", "--demo", "--enable-feature", "line-numbers"]).unwrap();
        assert_eq!(args.enable_feature, [crate::features::Feature::LineNumbers]);
        assert!(Args::try_parse_from(["nysos", "--enable-feature", "unknown"]).is_err());
        assert!(
            Args::try_parse_from([
                "nysos",
                "--init",
                "demo.toml",
                "--enable-feature",
                "line-numbers"
            ])
            .is_err()
        );
    }
    #[test]
    fn demo_is_opt_in_and_conflicts_with_other_sources() {
        assert!(!Args::try_parse_from(["nysos"]).unwrap().demo);
        assert!(
            Args::try_parse_from(["nysos", "--demo", "--check"])
                .unwrap()
                .demo
        );
        for other in [
            vec!["--config", "demo.toml"],
            vec!["--init", "demo.toml"],
            vec!["--add-to-path"],
            vec!["--install-completions"],
        ] {
            let mut args = vec!["nysos", "--demo"];
            args.extend(other);
            assert!(Args::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn version_switches_exit_with_build_information() {
        for flag in ["--version", "--version-full"] {
            let error = Args::try_parse_from(["nysos", flag]).err().unwrap();
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
            assert_eq!(error.exit_code(), 0);
            let report = error.to_string();
            for label in [
                "Build time (UTC):",
                "Git commit:",
                "Git tag:",
                "Git describe:",
                "Compiler:",
                "Target:",
                "Profile:",
            ] {
                assert!(report.contains(label), "{flag}: missing {label}");
            }
        }
        let short = Args::try_parse_from(["nysos", "-V"]).err().unwrap();
        assert_eq!(short.kind(), clap::error::ErrorKind::DisplayVersion);
        assert_eq!(
            short.to_string().trim(),
            concat!("nysos ", env!("CARGO_PKG_VERSION"))
        );
    }
    #[test]
    fn prefix_override_is_validated() {
        assert_eq!(
            Args::try_parse_from(["nysos", "--terminal-keys", "ghostty"])
                .unwrap()
                .terminal_keys,
            Some(crate::prefix::TerminalKeys::Ghostty)
        );
        assert!(Args::try_parse_from(["nysos", "--terminal-keys", "unknown"]).is_err());
        assert_eq!(
            Args::try_parse_from(["nysos", "--prefix", "ctrl-b"])
                .unwrap()
                .prefix,
            Some(crate::prefix::Prefix::CtrlB)
        );
        assert!(Args::try_parse_from(["nysos", "--prefix", "ctrl-space"]).is_err());
        assert!(Args::try_parse_from(["nysos", "--prefix", "f12", "--init", "demo.toml"]).is_err());
    }
    #[test]
    fn shell_setup_accepts_defaults_and_combination_but_not_demo_flags() {
        let args = Args::try_parse_from(["nysos", "--add-to-path", "--install-completions", "zsh"])
            .unwrap();
        assert_eq!(args.add_to_path, Some(ShellTarget::All));
        assert_eq!(args.install_completions, Some(ShellTarget::Zsh));
        assert!(Args::try_parse_from(["nysos", "--install-completions", "fish"]).is_err());
        assert!(Args::try_parse_from(["nysos", "--bin-dir", "/tmp"]).is_err());
        assert!(Args::try_parse_from(["nysos", "--add-to-path", "--check"]).is_err());
        assert!(
            Args::try_parse_from(["nysos", "--install-completions", "--init", "demo.toml"])
                .is_err()
        );
    }
    #[test]
    fn init_rejects_ignored_options_but_check_accepts_config() {
        assert!(Args::try_parse_from(["nysos", "--init", "a.toml", "--check"]).is_err());
        assert!(Args::try_parse_from(["nysos", "--init", "a.toml", "--config", "b.toml"]).is_err());
        assert!(Args::try_parse_from(["nysos", "--config", "a.toml", "--check"]).is_ok());
    }
}
