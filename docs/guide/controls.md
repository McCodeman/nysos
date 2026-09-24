<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

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
| `#` | Toggle the focused shell’s line-number gutter; requires the `line-numbers` gate |
| `e` | Edit the current queue item's name, description, targets, and commands |
| `o` | Edit the entire demo: title, add/reorder cues, panes, and layout |
| `a` | Add and focus an ad hoc shell |
| Tab / Right | Focus the next pane |
| Shift-Tab / Left | Focus the previous pane |
| `0` | Show and focus the cue list |
| `c` | Show/hide the cue list |
| `1`–`9` | Focus a shell pane directly |
| `l` | Cycle columns, rows, and grid; a custom tree first becomes columns |
| `<` / `>` | Narrow / widen focused pane or its nearest column group |
| `-` / `+` | Shorten / heighten focused pane or its nearest row group |
| `h` | Toggle the queue header |
| `x` | Restart the focused shell, ending its current session |
| `?` | Show help; Esc closes it |
| `q` | Quit and close shell sessions |

Press the prefix twice to forward that control character to the shell. Ctrl-C
reaches the focused shell without quitting nysos.

Click a pane to focus it. Drag a shared divider to resize adjacent panes.
In grid mode, horizontal widths are adjustable within each row; row heights
remain equal. Nested layouts support resizing inner panes and outer groups. Resizing changes
the active cue override when one is in effect, otherwise the global layout.
Font-size and terminal-window changes recompute proportions and propagate to every
PTY. Use `--no-mouse` and the keyboard resizing controls when capture is unwanted
or unavailable. See [layouts and resizing](layouts.md) for compatibility and limits.
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
| Enter | Send all remaining actions in the selected cue |
| `p` | Preview the selected cue’s command, keys/repeats, or clear action over each shell pane; Esc closes |
| `t` | Type the next command per target pane without Enter, then focus the first target |
| `s` | Restore the selected cue’s recorded layout and pane bookmarks |
| `b` | Restore the current live layout and return all panes to bottom |
| Space | Pause/resume an armed automatic-advance timer |
| `e` | Edit only the selected cue in the modal |
| `o` | Open the entire demo to add cues, rename the demo, or load/save a file |
| Tab / Shift-Tab | Cycle focus to a shell |
| Esc | Return to the last focused shell |

The mouse wheel browses the list; clicking selects without executing. The `▶`
marker identifies the next pending cue independently of your selection; its
`[1/2]` counter means command 1 of 2 is next, not that command 1 has run. The header
previews the selected cue while the list is focused. **Ctrl-G, c** toggles the
list, giving its space back to shell panes when hidden. On launch, the visible
cue list has focus. Set `cue_list = false` to start with it hidden and focus the
first shell instead; the default is `true`. Set `cue_list` and
`cue_width` in the configuration to customize it.

Enter resumes a partially sent current cue; selecting an earlier cue replays it.
After dispatch, selection advances to the next cue, or stays on the last cue.
Pressing Enter again on the last completed cue replays it. Commands are sent
in order without waiting for completion. Use **Ctrl-G, n** for one command at a
time when you need to wait between commands.

### Preview before executing

Select a cue and press **p**. A small dialog over each shell pane shows its first
remaining command from that cue and the number of pending commands for that pane.
Panes without a remaining command show “No pending command for this cue.” For the
current partially executed cue, the preview skips commands already sent or skipped;
other cues preview from their beginning, matching replay behavior.

**Up/Down** scrolls long command text in the preview dialogs. **Esc** dismisses
all dialogs and returns to the selected cue. Previews do not execute commands,
change playback progress, or send keyboard/paste/mouse input to the shells.
The shell processes continue running behind the dialogs. Close the preview before
pressing Enter to run the cue. With an empty cue list, `p` does nothing.

### Type and edit in the shells

With a cue selected, press **t**. nysos types the same next command shown by
preview into each target pane, without sending Enter, and focuses the first target
in pane order. Click another pane or rotate focus to edit its prepared line.
Press Enter in each shell when ready. A cue may target each pane only once;
`t` types that command if it has not already been sent or skipped.

Start with each target at an empty shell prompt. Typing inserts at its current
cursor; it does not clear existing input or interrupt a foreground program.
Commands containing control characters (including tabs) cannot be typed this way.
Pressing `t` again inserts the text again. Preview must be closed before using `t`.

Typing leaves cue progress unchanged because nysos cannot detect manual execution.
After submitting the edited lines, use prefix-s to skip those commands in playback,
or select a later cue. Avoid running the same cue through the queue until pending
shell input is submitted or cleared; Enter in the queue sends commands again.

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

### Key actions and automatic playback

For `keys` and `clear` actions, `t` sends the action immediately. If a cue contains
only those actions, it advances just like Enter. Mixed cues type shell commands
without Enter, send key/clear actions, and leave playback unchanged; later Enter
repeats every action. Prefix-n sends one action and prefix-s skips one action.
Cue-list `s` has a different meaning: restore output bookmarks.

Optional `advance_after_ms` timers execute the next cue; `loop = true` wraps to
the first cue. Space pauses/resumes an armed timer in the cue list. Preview,
help, and editor dialogs suspend it. Browsing/selecting a cue, restoring bookmarks,
or applying edits cancels playback. See the
[complete rules](../reference/configuration.md#key-actions-timers-and-native-clear).

The experimental [line-number gutter](feature-gates.md) is toggled during a
session with **prefix, #**. Click it to focus its pane; wheel over it to scroll
history. Gutter clicks and numbers never reach the child application.
