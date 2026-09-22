<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Presenting a demo

## Rehearse

Generate a starting file with `nysos --init demo.toml`, customize it using the
[configuration reference](../reference/configuration.md), then validate it:

```sh
nysos --config demo.toml --check
nysos --config demo.toml
```

Validation checks structure and pane references. It does not check whether your
shells or working directories exist, whether commands are valid, or whether
external services are ready. Run the demo once to rehearse those details.

## Advance the queue

A demo is an ordered list of queue items. Each item contains one or more commands,
and each command names its target pane. The header shows the item index, name,
description, and command index. Toggle it with **prefix, h**.

Press **Ctrl-G**, release it, then:

- **n** or **Enter** sends the next command plus Enter to its target pane.
- **s** skips that command without sending it.
- **e** opens the current queue item for editing before execution.

An advance sends one command, not the entire queue item. After its last command,
the next queue item becomes current. After the final item, the header says the
demo is complete; all panes remain interactive. There is no timing engine or
command-completion detection. Loading or applying a demo never runs queued
commands automatically.

Wait for the target shell's prompt before advancing. The command goes to its
current foreground program or unfinished input buffer. If an editor, REPL, or
long-running command is active, quit or interrupt it as appropriate before
sending another shell command. Ctrl-C goes to the focused pane.

## Browse and run cues

The left cue list lets you jump directly to an item. Click it or press
**Ctrl-G, 0**, browse with **Up/Down**, and press **e** to edit the selection.
**p** previews the next command for each pane in a small overlay; **Esc** closes
the previews without running anything. **t** types the next command per target
pane without Enter so you can edit it directly in the shell. Typing leaves
playback progress unchanged; see [controls](controls.md#type-and-edit-in-the-shells). **Enter** dispatches its commands in order without waiting for completion, then
selects the next cue. It resumes remaining commands for the current playback
item; choosing an earlier item replays it. Browsing alone changes no progress.
Use **Ctrl-G, c** to show/hide the list and **Esc** to return to your shell.

## Work interactively

Click a pane, press **Alt-Left/Right**, or use **prefix, Tab** to rotate focus.
Type normally. Tab completion, command history, and interactive CLI programs use
the real shell. Scripted commands target the named pane regardless of focus.

Use **prefix, a** to add and focus an `adhoc-N` pane using `$SHELL` (or `/bin/sh`).
Use **prefix, o** to rename/configure it or save it for a later demo. There may be
up to 16 panes. To remove one, delete its `[[panes]]` entry, remove its leaves from global and
per-cue layout trees, and update any commands that target it before applying the configuration.

## Adjust the view

Use **prefix, l** to cycle columns, rows, and grid, or edit `layout` in the modal.
Drag shared pane/group borders, or use prefix then `<`/`>` for width and `-`/`+`
for height. Grid rows stay equal; nested trees permit row and column resizing.
Font/window cell-size changes recalculate the layout and resize each PTY.
A cue's optional `layout` applies when its commands are executed or typed, not
previewed/skipped, and persists until another override or full-demo apply.
Resizing while an override is active edits that cue's layout; otherwise it edits
the global layout. See [layouts and resizing](layouts.md).
These changes remain in memory until you save through the modal.

## Restart or finish

**Prefix, x** restarts the focused shell, ending its current session. **Prefix, q**
quits nysos and closes its shells. Detached background jobs may outlive a pane.
There is no automatic save on exit.

To replay from the beginning, open **prefix, o** and apply the demo again. This
resets queue progress but retains sessions whose ID, shell, arguments, and
working directory match. Shell state is therefore not reset by replay; restart
panes or relaunch nysos if you need a fresh environment.
