<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Troubleshooting

| Symptom | What to do |
| --- | --- |
| “requires an interactive terminal” | Launch directly in a terminal, with both stdin and stdout attached. Use `--check` in scripts or CI. |
| Shell fails to start | Check the pane's `shell`, `args`, and `cwd`. `/bin/zsh` may not be installed on Linux; try `/bin/sh`. |
| Unknown pane in a command | Match the exact case-sensitive pane ID. Update command targets when changing pane IDs. |
| Prefix has no effect | Default is Ctrl-G. Check the active prefix in the footer; use `--prefix f12` if your multiplexer intercepts it. See [tmux and SSH](tmux-ssh.md). |
| Option/Alt-Left or Right does not rotate focus | Current nysos supports standard Alt-arrows and detects Ghostty’s default `Esc b` / `Esc f`. Use `--terminal-keys ghostty` if tmux/SSH masks the terminal identity, or `standard` to preserve shell Alt-B/F. Rebuild/reinstall if using an older binary; inspect `ghostty +list-keybinds` for custom actions. Use prefix-Tab or click if the terminal consumes the shortcut. |
| A command appears inside an editor/REPL | Queue commands go to the current foreground program. Return to a shell prompt before advancing. |
| A pane says `[exited]` | Focus it, then use prefix-x to start a new shell. |
| A tiny terminal hides content | Enlarge the terminal, toggle the header off with prefix-h, change layout with prefix-l, or remove panes through the editor. |
| Cmd-click does nothing | Use Alt-click. The terminal must forward clicks; the local Command-state fallback is disabled over SSH and inside tmux. The app opens URLs on the machine where it runs. Plain URLs wrapping across rows are not joined. |
| Scroll wheel reaches an application | Hold Shift to request scrollback; your host terminal may itself intercept Shift-mouse. |
| Edited commands run again | Applying a queue edit rewinds that item; applying a full demo rewinds all queue progress. Remove already-completed commands before applying a queue edit if needed. |
| Comments disappear after save | The modal serializes normalized TOML and does not preserve comments. Keep annotated templates separately. |
| Changes are gone after exit | Layout changes, ad hoc panes, and edits are in memory until saved through prefix-o, Ctrl-S. |
| Full-editor shortcut seems ignored | Use Control, not macOS Command, and no prefix inside the modal. Try F2/F3/F4 (Fn may be required). Save/Apply keeps the editor open for invalid TOML; read the error below the text. Confirm a load/save path with Enter. |
| Save failed | Check the destination's parent directory and write access. Paths are literal: use an absolute path instead of `~`. |

| Old demo or missing new features | Check `command -v nysos` and `nysos --version-full`. A Homebrew/system binary may be older than a Cargo build. Reinstall the method you intend to use and check PATH precedence. |
| Preview leaves colored blocks | Update to the pane-rendering fix; older releases emitted literal tabs that could corrupt redraws. |
| Mouse drag does nothing | Use `--no-mouse` and prefix `<`/`>` / `-`/`+`. For tmux enable `mouse on`; terminals can intercept reports. See [layouts](layouts.md). |
| Layout fails validation | Each global/cue tree must contain every pane ID exactly once; splits need at least two children and positive weights. |
| Layout changes on cue execution | That cue has a `layout` override. Omitted layouts retain the active arrangement; full-demo apply restores the global layout. |
| Installer reports missing assets | Older releases, including v0.1.1, have no prebuilt packages. Use source installation or a newer release with assets. |
| Installer reports checksum mismatch | Nothing is installed. Remove the downloaded archive and retry from the same release; do not bypass verification. |

`--check` prints a success summary and exits 0 for structurally valid demos; errors
exit 1. Invalid command-line options exit 2. See the [CLI reference](../reference/cli.md).

## Current limits

Custom RGB theme definitions are not supported, nor are
graphics protocols, clipboard escape-sequence integration, terminal text selection,
or automatic command-completion detection. Nested row/column trees are supported; preset grid rows have equal height. Direct shell sessions
are closed when their panes close; detached background jobs can remain running.
Native Windows interaction is deferred.
