<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# nysos

Present CLI demos with named, interactive shell panes and a prepared command
queue. Run or skip each command when you are ready, edit commands before sending
them, and open extra panes for questions or ad hoc exploration.

- [Get started](guide/getting-started.md): install nysos and walk through the three-pane demo.
- [Installation](guide/installation.md): Homebrew, source, Linux packages, and release installer.
- [Layouts](guide/layouts.md): nested rows/columns, global and per-cue sizing, mouse/keyboard resizing.
- [Present a demo](guide/presenting.md): move between prepared and interactive work.
- [Controls](guide/controls.md): keyboard shortcuts, pane resizing, and links.
- [Edit and save](guide/editor.md): use the configuration modal during a demo.
- [Configuration reference](reference/configuration.md): every TOML field and default.
- [CLI reference](reference/cli.md) and [manpage](reference/manpage.md): terminal help.

macOS is the primary platform; Linux is secondary and included in CI. Native
Windows interaction is deferred; use WSL. Each pane runs in a real PTY using
portable-pty, with output interpreted by Alacritty's terminal engine and drawn
with Ratatui. Crossterm handles host-terminal input. You do not need the Alacritty
application installed.
