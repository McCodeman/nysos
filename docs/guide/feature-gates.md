<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Feature gates and line numbers

Experimental features use explicit runtime gates. The `line-numbers` gate
defaults to **on**, but line numbers start **hidden**, including in the built-in demo; no special Cargo build is needed. They are available only in builds whose help
lists them. From a source checkout, run `make install` to update your local binary.

```sh
nysos --demo
```

If line numbers cause problems, disable the feature at launch:

```sh
nysos --demo --disable-feature line-numbers
```

Omitting `features` in TOML uses the default enabled gates. An explicit
`features = []` disables all gates; an explicit array replaces the default list.
`line_numbers = false` only hides gutters while keeping the feature available
for live toggling.

`--enable-feature FEATURE` and `--disable-feature FEATURE` are repeatable. The
only current feature name is `line-numbers`. Unknown names are rejected by both
the CLI and TOML parser. Enables are added to the demo's `features` array;
disables then remove them, so disabling wins even if an enable appears later on
the command line. These are startup overrides, not permanent locks: the full
demo editor can subsequently change the list. Saving includes the effective list.

```toml
features = ["line-numbers"]
line_numbers = true

[[panes]]
id = "presenter"
line_numbers = true

[[panes]]
id = "observer"
line_numbers = false

[[queues]]
name = "Number observer output"
commands = [
  { pane = "observer", command = "pwd", line_numbers = true },
]
```

The top-level `line_numbers` setting defaults to `false`, but has **no effect
without the gate**. A pane's optional setting overrides that default; a cue action
can change that pane's visibility when dispatched or typed. Omission retains its
current setting. Preview and skip never apply the override. Gate-off ignores all
visibility settings, including cue overrides. To disable the entire feature:

```sh
nysos --config demo.toml --disable-feature line-numbers
```

## Show or hide during a session

Focus a shell and press **Ctrl-G**, release, then **#** (Shift-3 on a US keyboard).
The first press shows the initially hidden gutter; the next hides it. This toggles that pane's gutter immediately, without restarting the shell. It
requires the gate; a disabled gate produces an explanatory status message.
With a custom prefix, use that prefix followed by # instead.

The toggle also changes the pane's initial setting in the editable configuration.
Save through the full editor to persist it. Subsequent cues can override it again.
Use the full editor's top-level `line_numbers` setting to change the default for
all panes without explicit overrides, or remove `line-numbers` from `features`
to disable every gutter. Applying full configuration follows the usual rules:
it resets cue playback and preserves matching shell processes.

## An attached, synchronized gutter

```text
┌ Presenter ──────────────────────┐
│     41 │ $ make build           │
│     42 │ Compiling a long line  │
│      ↳ │ continued output       │
│     43 │ Finished               │
└────────────────────────────────┘
```

The eight-column gutter belongs to nysos, outside the PTY's content. The child
terminal receives the remaining width, so wrapping, cursor placement, links,
and application mouse coordinates agree. Numbering uses logical buffer lines;
soft-wrapped continuations display `↳`. Blank lines are numbered too.

There is no second scroll state: gutter labels use the terminal's current
scrollback offset. Wheel scrolling, cue bookmarks (`s`), and return-to-bottom
(`b`) stay synchronized. Clicking the gutter focuses its pane. Wheel events
over it always scroll history; gutter clicks never reach the application or open
URLs. The pane's outer border remains the resize handle.

The gutter uses the theme's high-contrast foreground. Its width stays fixed as
numbers grow. If a pane has fewer than 16 interior columns, the gutter collapses
to preserve terminal space and returns when widened. Showing/hiding or collapsing
it resizes the PTY; existing output bookmarks and retained line numbers follow
the reflow. Cue visibility changes happen before recording that cue's new bookmark.

Alternate-screen applications such as Vim get a blank reserved gutter, without
numbers or a separator. Its width stays reserved to avoid another PTY resize when
the application enters or leaves its alternate screen. Numbers resume on return
to the normal buffer.

Numbers never enter shell input or process output. Host-terminal drag selection
may still copy the rendered gutter; nysos does not currently provide a separate
copy command that strips it.

## Experimental numbering limits

Numbers start at 1 for the retained normal buffer when the gate is enabled.
Normal scrolling and history eviction preserve the remaining line numbers;
native clear preserves history and numbering. Ordinary soft wrapping and reflow
preserve logical numbering. Restarting the shell, clearing history, disabling and
re-enabling the gate, or an unsupported terminal buffer rearrangement starts a
new numbering sequence. Layout changes and reflow preserve the numbers of retained
logical lines, including at history capacity and while an alternate-screen
application is active. Only positions actually discarded from history are lost.

Programs can overwrite text, insert/delete rows, or change scrolling regions.
Numbers describe the current terminal buffer, not an immutable transcript or
persistent output IDs. Editing wrap boundaries can change subsequent numbering.
Numbers above 999999 show `++++++`, keeping the gutter width stable.

The source example `examples/line-numbers.toml` demonstrates wrapping and per-cue
show/hide overrides:

```sh
nysos --config examples/line-numbers.toml
```
