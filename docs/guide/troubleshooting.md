# Troubleshooting

| Symptom | What to do |
| --- | --- |
| “requires an interactive terminal” | Launch directly in a terminal, with both stdin and stdout attached. Use `--check` in scripts or CI. |
| Shell fails to start | Check the pane's `shell`, `args`, and `cwd`. `/bin/zsh` may not be installed on Linux; try `/bin/sh`. |
| Unknown pane in a command | Match the exact case-sensitive pane name. If you replaced the built-in panes, specify your own `queues` or set `queues = []`. |
| Ctrl-B has no effect | Check host-terminal shortcuts. In default tmux, press Ctrl-B twice to send the prefix through to nysos. |
| Alt-arrows do not rotate focus | Use prefix-Tab or click. On macOS, configure Option as Alt/Meta in the host terminal. |
| A command appears inside an editor/REPL | Queue commands go to the current foreground program. Return to a shell prompt before advancing. |
| A pane says `[exited]` | Focus it, then use prefix-x to start a new shell. |
| A tiny terminal hides content | Enlarge the terminal, toggle the header off with prefix-h, change layout with prefix-l, or remove panes through the editor. |
| Cmd-click does nothing | Use Alt-click. The terminal must forward clicks; the local Command-state fallback is disabled over SSH. Plain URLs wrapping across rows are not joined. |
| Scroll wheel reaches an application | Hold Shift to request scrollback; your host terminal may itself intercept Shift-mouse. |
| Edited commands run again | Applying a queue edit rewinds that item; applying a full demo rewinds all queue progress. Remove already-completed commands before applying a queue edit if needed. |
| Comments disappear after save | The modal serializes normalized TOML and does not preserve comments. Keep annotated templates separately. |
| Changes are gone after exit | Layout changes, ad hoc panes, and edits are in memory until saved through prefix-o, Ctrl-S. |
| Save failed | Check the destination's parent directory and write access. Paths are literal: use an absolute path instead of `~`. |

`--check` prints a success summary and exits 0 for structurally valid demos; errors
exit 1. Invalid command-line options exit 2. See the [CLI reference](../reference/cli.md).

## Current limits

This scaffold does not support arbitrary split trees, custom RGB theme definitions,
graphics protocols, clipboard escape-sequence integration, terminal text selection,
or automatic command completion. Grid rows have equal height. Direct shell sessions
are closed when their panes close; detached background jobs can remain running.
Native Windows interaction is deferred.
