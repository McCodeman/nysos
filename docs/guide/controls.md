# Controls and mouse

When a shell is focused, normal typing, Enter, Tab, Ctrl-C, arrows, and function keys go to the focused
shell. **Alt-Left/Right** rotates focus. On macOS, configure Option as Alt/Meta
in your terminal if needed, or use prefix-Tab.

On macOS, Ghostty's default Option-Left/Right bindings send `Esc b` / `Esc f`,
which arrive as Alt-B/F rather than arrow events. In the Ghostty profile, nysos accepts both these
sequences and standard Alt-Left/Right. As a result, Alt-B/F also changes focus
instead of shell word navigation while the main view is active. Modal editors
keep their own key handling. Only left/right arrows rotate focus; Alt-Up/Down
are forwarded to the focused shell.

The default `terminal_keys = "auto"` profile detects Ghostty from
`TERM_PROGRAM=ghostty` (case-insensitive) or, when the program is absent/tmux,
`TERM=xterm-ghostty`. Unknown terminals use `standard`, preserving Alt-B/F shell
word navigation. Other named programs take precedence over TERM. The active
profile appears in the startup status and help title.

Override detection with `--terminal-keys ghostty` or `--terminal-keys standard`,
or save `terminal_keys = "ghostty"` in the demo. CLI settings take precedence at
startup; the full config editor can change them later. Auto detection reads the
launch environment, not physical keybindings: custom bindings or stale tmux
environments can require an override. Detection does not change during a tmux
reattach; apply a profile through the editor if the client terminal changes.

No Ghostty configuration change is required for its default bindings. To inspect
custom bindings, run `ghostty +list-keybinds`; a shortcut consumed by Ghostty or
macOS cannot reach nysos. See [Ghostty keybinding actions](https://ghostty.org/docs/config/keybind/reference).


The default prefix is **Ctrl-G**. Override it with `--prefix` or the demo
`prefix` field; substitute your chosen key in the controls below. See
[tmux and SSH](tmux-ssh.md) for remote sessions and mouse forwarding.

After **Ctrl-G**:

| Key | Action |
| --- | --- |
| `n` or Enter | Send the next command, then advance |
| `s` | Skip the next command |
| `e` | Edit the current queue item's name, description, targets, and commands |
| `o` | Edit the entire demo: title, add/reorder cues, panes, and layout |
| `a` | Add and focus an ad hoc shell |
| Tab / Right | Focus the next pane |
| Shift-Tab / Left | Focus the previous pane |
| `0` | Show and focus the cue list |
| `c` | Show/hide the cue list |
| `1`–`9` | Focus a shell pane directly |
| `l` | Cycle columns, rows, and grid |
| `h` | Toggle the queue header |
| `x` | Restart the focused shell, ending its current session |
| `?` | Show help; Esc closes it |
| `q` | Quit and close shell sessions |

Press the prefix twice to forward that control character to the shell. Ctrl-C
reaches the focused shell without quitting nysos.

Click a pane to focus it. Drag a shared divider to resize adjacent panes.
In grid mode, horizontal widths are adjustable within each row; row heights
remain equal. Terminal-window resizes propagate to every PTY.
The wheel scrolls terminal history, or is forwarded to applications that
request mouse input. Shift-wheel requests history scrolling, though some host
terminals reserve Shift-mouse for their own selection/scrollback.

## Cue list

The narrow left sidebar is visible by default. Click it, cycle focus into it, or
press **Ctrl-G, 0**. While focused:

| Key | Action |
| --- | --- |
| Up / Down | Select the previous / next cue without executing |
| Home / End | Select the first / last cue |
| Page Up / Page Down | Browse a page at a time |
| Enter | Send all remaining commands in the selected cue |
| `e` | Edit only the selected cue in the modal |
| `o` | Open the entire demo to add cues, rename the demo, or load/save a file |
| Tab / Shift-Tab | Cycle focus to a shell |
| Esc | Return to the last focused shell |

The mouse wheel browses the list; clicking selects without executing. The `▶`
marker identifies playback progress independently of your selection. The header
previews the selected cue while the list is focused. **Ctrl-G, c** toggles the
list, giving its space back to shell panes when hidden. Set `cue_list` and
`cue_width` in the configuration to customize it.

Enter resumes a partially sent current cue; selecting an earlier cue replays it.
After dispatch, selection advances to the next cue, or stays on the last cue.
Pressing Enter again on the last completed cue replays it. Commands are sent
in order without waiting for completion. Use **Ctrl-G, n** for one command at a
time when you need to wait between commands.

## Links on macOS

**Cmd-click** opens an HTTP(S) URL in the default browser. Standard terminal mouse
reports omit Command; for local macOS sessions nysos samples the
[CoreGraphics modifier flags](https://developer.apple.com/documentation/coregraphics/cgeventflags)
when a click arrives. No event tap or keyboard recording is installed. A terminal
that consumes Cmd-click may open the link itself; nysos cannot handle clicks the
host does not forward. This native modifier fallback is disabled over SSH and inside tmux. URL openers
run on the nysos host, so a remote nysos cannot open your client browser.

**Alt-click** is the portable fallback. The app recognizes OSC 8 hyperlinks and
plain URLs on a visible terminal row. Plain URLs wrapping across rows are not
joined. Only `http` and `https` are opened, through `open` on macOS or `xdg-open`
on Linux; URLs are passed as process arguments, never through a shell.


See [editing and saving](editor.md) for shortcuts inside the modal.
