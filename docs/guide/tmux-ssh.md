<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# tmux and SSH

The default nysos prefix is **Ctrl-G**, followed by a command key. It avoids
[tmux's default Ctrl-B prefix](https://man.openbsd.org/tmux#DEFAULT_KEY_BINDINGS).
For example, **Ctrl-G, 0** focuses the cue list, **Ctrl-G, n** sends a command,
and **Ctrl-G, q** quits nysos. Ctrl-Space is not used.

## Run inside tmux

```sh
tmux new-session -s demos
nysos --config demo.toml
```

Use Ctrl-B for tmux and Ctrl-G for nysos. Enable tmux mouse forwarding if you
want nysos's click-to-focus and divider dragging:

```sh
tmux set-option -g mouse on
```

This affects that tmux server. Add `set -g mouse on` to your own tmux config only
if you want it for future servers. tmux's own borders/status bar remain controlled
by tmux. Inside the nysos region, mouse reports reach nysos. Leave tmux copy mode
to resume interacting with the app. Terminal resizes propagate through tmux to
the shell panes.

## Choose another prefix

```sh
nysos --prefix f12
nysos --prefix ctrl-b --config demo.toml
```

Supported values are `ctrl-g` (default), `ctrl-a`, `ctrl-b`, and `f12`. Choose one
that your terminal and outer multiplexers forward. You can also save a top-level
setting in the demo:

```toml
prefix = "ctrl-g"
```

The CLI overrides the file at startup. The full demo editor can change it later;
saving the demo persists the effective prefix. On-screen hints show the active
prefix. Modal editor shortcuts, including Ctrl-G to apply, stay unchanged.
Press the chosen prefix twice to send one literal prefix key to a focused shell.

If you keep nysos on Ctrl-B inside default tmux, press **Ctrl-B, Ctrl-B, n** to
advance nysos: tmux's `send-prefix` binding forwards one Ctrl-B to the app.
Sending a literal Ctrl-B to nysos's shell then takes four Ctrl-B presses. Using
a distinct prefix avoids this extra layer. A custom tmux prefix may need a
different choice; nysos does not change your tmux bindings.

## Terminal-specific arrow mappings

Auto mode detects Ghostty from `TERM_PROGRAM` or `TERM` when that identity is
available. tmux and SSH can hide or retain a stale identity, so for Ghostty use:

```sh
nysos --terminal-keys ghostty --config demo.toml
```

This handles Ghostty Option-arrows (`Esc b/f`) as focus commands, including over
SSH to Linux. Use `--terminal-keys standard` to preserve Alt-B/F as shell word
navigation. Standard Alt-arrow sequences work in both profiles. Do not change
`TERM` just to select nysos mappings. The selected profile is shown in help.

## Run on a remote machine

Install nysos on the remote host, then allocate a terminal with SSH:

```sh
ssh -t demo-host
# On the remote host:
tmux new-session -A -s demos
nysos --config demo.toml
```

Alternatively, run nysos directly with `ssh -t demo-host 'nysos --config demo.toml'`.
Both input and output must be terminals. nysos does not need a desktop session,
Command-key forwarding, or an extended keyboard protocol for its controls.
Use prefix-Tab when Alt-arrows are intercepted by your terminal. Shells and paths
belong to the remote host. Install an appropriate terminfo entry on that host if
SSH/tmux reports an unknown terminal; keep the terminal type truthful.

A remote tmux session survives SSH disconnection; reconnect and run
`tmux attach-session -t demos`. nysos itself is not a session server: a direct SSH
session without tmux does not promise survival across disconnects.

### Remote URLs

nysos's URL opener runs on the machine running nysos. It does not open a browser
on your SSH client. For a local browser, use the host terminal's link detection
and Cmd-click/copy the URL there. Alt-click invokes the remote machine's opener
and requires an available browser environment there. Local macOS Command-state
sampling is disabled over SSH and inside tmux, where the client can change.

## Verification

`nix develop --command make test-tmux` uses an isolated tmux server and a real
PTY client to test default and legacy prefixes, cue navigation/editing/execution,
standard Alt-arrow and Ghostty Option-arrow focus, mouse forwarding, window
resize, and clean exit. `make test-focus` checks the same focus encodings directly
through a PTY and verifies which shell receives subsequent input. It sets SSH environment markers;
it does not establish a network SSH connection. The test does not alter your tmux
configuration or sessions.
