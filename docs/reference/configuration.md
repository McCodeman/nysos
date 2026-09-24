<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Demo configuration reference

A demo is a UTF-8 TOML file selected with `nysos --config PATH` or loaded through
the modal. Plain `nysos` starts two interactive panes with no cues; `nysos --demo`
explicitly loads the built-in twelve-cue example. There is no automatic discovery of `demo.toml`. Unknown fields and
invalid enum values are rejected. All paths resolve relative to nysos's launch
directory, not the configuration file's directory; `~` and environment variables
inside configured paths are not expanded.

## Configuration structure

The tables below list **every supported field**. Field names and enum strings
are case-sensitive. Booleans are unquoted `true` or `false`; integers are
unquoted whole numbers. Unknown keys are rejected at every level, including
inside panes, queues, and commands.

| Location | All valid keys |
| --- | --- |
| Top level | `features`, `line_numbers`, `title`, `prefix`, `terminal_keys`, `header`, `cue_list`, `cue_width`, `loop`, `layout`, `panes`, `queues` |
| Each `[[panes]]` | `id`, `name` (legacy alias for `id`), `title`, `shell`, `args`, `cwd`, `scheme`, `weight`, `line_numbers` |
| Each `[[queues]]` | `name`, `description`, `commands`, `layout`, `advance_after_ms` |
| Each command in a queue | `pane`, `command`, `keys`, `clear`, `title`, `scheme`, `line_numbers` |
| Each item in `keys` | `key`, `repeat` |

A **cue** in the UI is a `[[queues]]` item in TOML; `cues` is not an alias.
Pane count comes from the number of `[[panes]]` entries, not a `pane_count`
setting. A pane's `id` is its stable command target; `title` is its display label
and defaults to the ID. Legacy `name` is accepted as an alias for `id`; do not
specify both. `scheme` selects a built-in theme; there is no `theme`, `color`, or
custom RGB field. Keyboard mapping settings belong at the top level, not inside panes.

## Complete example

This standalone example includes every supported field and uses `/bin/sh` on
both macOS and Linux. `cwd = "."` means the nysos launch directory:

```toml
title = "Service walkthrough"
prefix = "ctrl-g"
terminal_keys = "auto"
header = true
cue_list = true
cue_width = 26
layout = "columns"

[[panes]]
id = "presenter"
title = "Presenter shell"
shell = "/bin/sh"
args = []
cwd = "."
scheme = "ocean"
weight = 3

[[panes]]
id = "observer"
shell = "/bin/sh"
cwd = "/tmp"
scheme = "ember"
weight = 2

[[queues]]
name = "Welcome"
description = "Introduce the two independent shells."
commands = [
  { pane = "presenter", command = "printf 'Welcome to nysos!\\n'" },
  { pane = "observer", command = "pwd" },
]

[[queues]]
name = "Explore"
description = "Edit this item before running it if needed."
commands = [
  { pane = "presenter", command = "ls -lah", title = "Project files", scheme = "forest" },
  { pane = "observer", command = "printf 'https://ratatui.rs\\n'" },
]
```

## Top-level fields

| Field | Type | Default when omitted | Meaning |
| --- | --- | --- | --- |
| `features` | Array of string enums | `["line-numbers"]` | Enabled experimental gates. Currently only `"line-numbers"`; use `[]` to disable all gates. An explicit array replaces the defaults. Unknown names fail validation. CLI enables merge into this list, then disables remove entries. |
| `line_numbers` | Boolean | `false` | Default gutter visibility, effective only with the `line-numbers` gate. Individual panes may override it. |
| `title` | String | `"nysos demo"` | Nonempty demo title displayed above cue details when the header is enabled; control characters are rejected. |
| `terminal_keys` | String enum | `"auto"` | `"auto"` detects Ghostty using TERM_PROGRAM/TERM; `"ghostty"` adds Alt-B/F focus aliases; `"standard"` preserves those shell keys. `--terminal-keys` overrides at startup. |
| `prefix` | String enum | `"ctrl-g"` | Control prefix: `"ctrl-g"`, `"ctrl-a"`, `"ctrl-b"`, or `"f12"`. `--prefix` overrides it at startup. |
| `header` | Boolean | `true` | Show the demo title, cue index/name/description, and total command count above the panes. When the cue list is focused, show its selection. |
| `cue_list` | Boolean | `true` | Initially show and focus the left cue sidebar. Set `false` to hide it and focus the first shell. Toggle with Ctrl-G, c. |
| `loop` | Boolean | `false` | Wrap playback to the first cue after the last. Does not start playback automatically. |
| `cue_width` | Integer | `26` | Sidebar width including borders, 16–60 columns; capped at half the content width in small terminals. |
| `layout` | String or layout table | `"columns"` | Preset `"columns"`, `"rows"`, `"grid"`, or a nested split tree (below). |
| `panes` | Array of pane tables | Built-in `presenter` and `observer` panes | Between 1 and 16 pane definitions; their count determines pane count. |
| `queues` | Array of queue tables | `[]` | Ordered queue items; an empty array is allowed. |

Omitting `queues` gives an empty cue list, including when defining custom panes.
An empty TOML file describes two interactive shells with no cues; `panes = []`
is invalid. No sample commands are inserted into files that omit `queues`.

The omitted `panes` array creates `presenter` (ocean) and `observer` (ember),
each with weight 1 and the normal shell/args/cwd defaults.
`nysos --demo` explicitly loads the twelve-cue pane transitions example, using
`service`, `observer`, and `notes` panes with `/bin/sh`. Service fills the left
column; Observer and Notes are initially stacked in the right column. It runs
portable, bounded commands without network access, changes layouts, hides and
restores live panes, and ends with a practice checklist for bookmarks and editing.
Only cues 5, 8, and 10 change layouts; the other nine cues inherit their arrangement.
Every cue is manually advanced; wait for shell prompts before continuing.
`nysos --init demo.toml` exports this example; it refuses to overwrite a file.

For a purely interactive single-pane demo:

```toml
queues = []

[[panes]]
id = "scratch"
```

Put top-level assignments before the first `[[panes]]` or `[[queues]]` table;
TOML assignments after a table header belong to that table.

### Prefix values

| Value | Prefix key |
| --- | --- |
| `"ctrl-g"` | Ctrl-G (default; distinct from default tmux Ctrl-B) |
| `"ctrl-a"` | Ctrl-A |
| `"ctrl-b"` | Ctrl-B (legacy nysos binding) |
| `"f12"` | F12 |

Press and release the prefix, then the action key. Press it twice to forward one
literal prefix to the focused shell. Ctrl-Space is not an accepted value.
Shortcuts in these docs use the default Ctrl-G; substitute your chosen prefix.
Modal shortcuts such as Ctrl-G/F4 to apply remain unchanged.

### Terminal key profile values

| Value | Behavior |
| --- | --- |
| `"auto"` | Detect once from the environment when nysos starts (default). |
| `"standard"` | Alt-Left/Right rotates focus; Alt-B/F is forwarded to the shell. |
| `"ghostty"` | Alt-Left/Right and Alt-B/F rotate focus; handles Ghostty's default macOS Option-arrow sequences. |

Auto chooses Ghostty when `TERM_PROGRAM` equals `ghostty` (ignoring case).
If `TERM_PROGRAM` is absent, empty, or exactly `tmux`, `TERM=xterm-ghostty` also
selects Ghostty. All other cases select standard, including an explicitly named
other terminal with a conflicting TERM value. tmux/SSH can hide or retain stale
environment values; use an explicit profile when necessary. The detected profile
is not refreshed on tmux reattachment. The active profile appears in help.

`--prefix` and `--terminal-keys` override their TOML values at startup. Editing
or loading a full demo afterward uses that demo's values, and saving writes the
current values. Feature gates also have startup `--enable-feature` / `--disable-feature` overrides. Other configuration fields have no corresponding CLI overrides.
See [tmux and SSH](../guide/tmux-ssh.md) for examples.

## Pane fields: `[[panes]]`

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `id` | String | `"shell"` | Nonempty, unique, case-sensitive identity; control characters are rejected. Commands target this value. |
| `name` | String | Alias for `id` | Legacy spelling, accepted for compatibility. Specifying both `id` and `name` is invalid; saving writes `id`. |
| `title` | String | Pane ID | Initial display title; nonempty with no control characters. Titles need not be unique. |
| `shell` | String | `$SHELL`, or `"/bin/sh"` if unavailable | Nonempty executable path or name. Use `args` for arguments. |
| `args` | Array of strings | `[]` | Arguments passed directly to the shell executable, e.g. `["-l"]`. |
| `cwd` | String | Inherit launch directory | Working directory for this pane. Omit for inheritance; TOML has no null value. |
| `scheme` | String enum | `"ocean"` | `"ocean"`, `"ember"`, `"forest"`, or `"mono"`. |
| `line_numbers` | Boolean | Inherit top-level setting | Initial gutter visibility for this pane; requires the `line-numbers` gate. |
| `weight` | Integer | `1` | Relative size from 1 through 1000 for preset layouts. Nested layouts use weights on tree nodes instead. |

Give every pane an explicit ID: two panes that omit `id` (and its legacy alias
`name`) both become `shell` and fail validation. IDs are matched exactly, without
trimming or case folding. An ID that contains only whitespace is rejected.
Changing a title preserves command targets and shell sessions; changing an ID
requires updating command targets and starts a replacement session on apply.

Shell paths and working directories are checked only when a PTY starts, not by
`--check`. Shell commands do not belong in `shell`: use `shell = "/bin/zsh"` and
`args = ["-l"]`, rather than `shell = "/bin/zsh -l"`. A shell name such as
`"bash"` is resolved using PATH. No arguments, including login flags, are added
implicitly. The `$SHELL` fallback applies when that environment variable is
unavailable; an explicitly empty shell value fails validation. An explicit `cwd`
must be an existing directory when the pane starts.

Panes inherit the process environment; there is no per-pane `env` table. To set
variables for later commands, send shell `export` commands or invoke a wrapper
script as the shell. Every PTY receives
`TERM=xterm-256color` and `COLORTERM=truecolor`.

### Layout and weights

- `columns`: panes appear side by side; weights control widths.
- `rows`: panes appear top to bottom; weights control heights.
- `grid`: panes fill rows in configuration order with `ceil(sqrt(pane_count))`
  columns. Rows have equal height, and weights control widths within each row.
  A partially filled last row still uses its full width.

Weights are ratios, not percentages or fixed sizes. Two column panes weighted
3 and 2 occupy approximately 60% and 40% of the available width, including their
borders, after space for the header, footer, and cue list has been removed. Dragging a divider updates weights; save the demo to persist them.

### Nested layout trees

Instead of a preset string, use a table for `layout`. A node is either a pane
reference or a split; rows and columns can nest in any combination. For example,
this places a presenter on the left and stacks logs above notes on the right:

```toml
[layout]
direction = "columns"
children = [
  { pane = "presenter", weight = 2 },
  { direction = "rows", children = [{ pane = "logs" }, { pane = "notes" }] },
]
```

Add `[[panes]]` definitions for those three IDs and then your `[[queues]]`.
`layout = { direction = "columns", children = [...] }` is also valid inline-table
syntax. Do not define both the preset string and `[layout]` in the same file.
TOML fields after `[layout]` belong to it; put other top-level settings before it.

| Node | Field | Type / default | Meaning |
| --- | --- | --- | --- |
| Pane | `pane` | Required string | Exact stable pane ID; not its display title. |
| Pane or split | `weight` | Integer, default `1` | Relative share of its parent's width for columns or height for rows, from 1 through 1000. The root weight is validated but has no sizing effect. |
| Split | `direction` | Required string | `"columns"` for left-to-right children; `"rows"` for top-to-bottom children. `"grid"` is only a preset, not a split direction. |
| Split | `children` | Required array | Between 2 and 16 child nodes, each another pane reference or split. |

Every configured pane must appear exactly once in the top-level layout.
Cue layouts may omit panes to hide them; unknown and duplicate IDs are rejected
in both. Pane nodes cannot also have `direction` or `children`; split
nodes cannot have `pane`. Unknown fields are rejected. Maximum nesting depth is
16 edges below the root. A single-pane tree can be `layout = { pane = "scratch" }`.

Tree weights belong to sibling groups: a child split's weight sizes that entire
group; its children's weights divide the resulting space. `[[panes]].weight`
is ignored for tree geometry, but remains validated and available if you switch
to a preset. The cue sidebar, header, and footer are outside the tree.

Dragging nested dividers or using keyboard resizing changes node weights.
Saving through the full editor persists the tree. The same schema may be used
in a cue's optional `layout` field. Pane numbers and keyboard focus
order still follow the `[[panes]]` array, even when its order differs from the tree.
Adding an ad hoc pane appends it to a root column split with the average sibling
weight, or wraps any other root with a new equally weighted column to the right.
Ctrl-G, l replaces a custom tree with the columns preset.
See [layouts and resizing](../guide/layouts.md) for a five-pane example, font-size
behavior, keyboard controls, and terminal/mouse compatibility.

### Color schemes

| Scheme | Appearance |
| --- | --- |
| `ocean` | Cool foreground, dark blue background, cyan focus border |
| `ember` | Warm foreground, dark red background, orange focus border |
| `forest` | Pale green foreground, dark green background, green focus border |
| `mono` | Gray foreground, near-black background, white focus border |

Schemes control default pane colors and the focus border. Titles use a bold,
high-contrast theme foreground independently of focus; all built-in title/background
pairs exceed a 7:1 contrast ratio. Programs' ANSI colors
are rendered separately. Custom RGB theme definitions are not supported yet.

## Queue fields: `[[queues]]`

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `name` | String | Required | Label in the cue list and header; cannot be empty or whitespace-only. Cue names need not be unique. |
| `description` | String | `""` | Free-form presenter-facing text in the header; empty or multiline text is accepted, but the fixed-height header can clip long text. |
| `commands` | Array of action tables | Required | One or more actions, dispatched in listed order; each pane may appear at most once per cue. |
| `advance_after_ms` | Integer | No timer | 1–86400000 milliseconds after dispatch completes, execute the next cue. At the end, wrap only if `loop = true`. |
| `layout` | Preset string or layout tree | Keep the active layout | Same schema as top-level `layout`. Applies on first dispatched command or `t`, with node weights controlling per-cue sizing. |

Ctrl-G, n sends a single command. Enter in the cue sidebar sends all remaining
commands of the selected item in order, without waiting for completion. Commands
can target different panes within the same item. Skipping advances
past one command; finishing the last command advances to the next queue item.
There is no configured limit on the number of cues. Each cue has at most one
command per configured pane, so its command count cannot exceed the pane count.
The same pane may be targeted again in another cue.
`queues = []` is valid; `commands = []` within a cue is invalid. Shell commands
are sent with Enter to the target pane's current foreground program, not always
to a fresh shell. Do not assume completion before sending the next command.

## Command fields

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `pane` | String | Required | Exact ID of an existing pane; never its display title. |
| `command` | String | Absent | Nonempty single-line shell input; NUL, CR, and LF are rejected. Mutually exclusive with `keys` and `clear = true`. |
| `keys` | Array of key tables | `[]` | Ordered key presses; each table has required `key` and optional `repeat`. Mutually exclusive with command/clear. |
| `line_numbers` | Boolean | Keep current pane setting | Show/hide the target gutter before dispatch or typing; requires the `line-numbers` gate. Resizes the PTY before bookmarking/sending input. |
| `clear` | Boolean | `false` | Clear the pane terminal display directly, without sending shell input. Mutually exclusive with command/keys. |
| `title` | String | Keep current pane title | Change the target pane’s display title when dispatched; nonempty with no control characters. |
| `scheme` | String enum | Keep current pane scheme | Change the target pane’s theme: `"ocean"`, `"ember"`, `"forest"`, or `"mono"`. |

Title and scheme overrides apply when the command is sent (Enter in the cue list
or Prefix, n), or typed without execution using `t`. They affect only the target
pane and preserve its ID, shell, and scrollback. Subsequent cues keep the current
values unless they explicitly override them. Browsing, previewing, and skipping
commands do not apply styling. If jumping between cues, specify both fields when
a cue needs a particular appearance regardless of the previously executed cue.
Exactly one action is required: a nonempty `command`, nonempty `keys`, or
`clear = true`. Styling-only entries are not supported. The one-entry-per-pane
rule also applies to key and clear actions.

Runtime overrides are not written back into the initial `[[panes]]` definitions.
Saving preserves the initial pane configuration and the overrides in each cue.
Applying a full demo restores its initial titles and schemes while retaining
matching shell sessions. Restarting a pane retains its current appearance.

There is no per-command delay, environment, timeout, working directory, or
completion condition. Commands run in the target pane's existing shell state.
To trigger commands separately in the same pane, put them in additional cues.
To run multiple shell commands in one pane from a single cue, combine them into
one `command` entry using shell separators, or invoke a script. For example:

```toml
[[queues]]
name = "Inspect workspace"
commands = [
  { pane = "presenter", command = "cd /tmp && pwd && ls" },
  { pane = "observer", command = "date; uname -a" },
]
```

In sh, Bash, and Zsh, `&&` runs the next command only if the previous one succeeds;
`;` runs the next command regardless of its exit status. Choose syntax supported
by the pane's configured shell. These compound entries still count as one command
per pane in the cue header. Enter in the cue list dispatches both entries without
waiting for either pane to finish.

TOML escaping and shell escaping are separate. In a double-quoted TOML string,
`\\n` becomes literal backslash-plus-`n` for `printf`; `\n` becomes a real newline
and fails command validation. Literal TOML strings avoid backslash processing:

```toml
[[queues]]
name = "Escaping"
commands = [
  { pane = "presenter", command = 'printf "hello\n"' },
]
```

### Nested command tables

Commands may be inline tables (as above) or `[[queues.commands]]` tables. This
standalone example uses the latter, with multiple commands in one cue:

```toml
title = "Nested command example"

[[panes]]
id = "shell"
shell = "/bin/sh"

[[panes]]
id = "files"
shell = "/bin/sh"

[[queues]]
name = "Inspect"
description = "One command for each of two independent shells."

[[queues.commands]]
pane = "shell"
command = "pwd"

[[queues.commands]]
pane = "files"
command = "ls -lah"

[[queues]]
name = "Finish"
commands = [{ pane = "shell", command = "printf 'Done\\n'" }]
```

Each `[[queues.commands]]` belongs to the most recent `[[queues]]`. Put the cue's
`name` and `description` before its command tables. Do not also define a
`commands = [...]` array for the same cue; TOML rejects duplicate definitions.

## Validation and applying changes

```sh
nysos --config demo.toml --check
```

Validation rejects malformed TOML, wrong value types, unknown fields/enum values,
blank titles or titles containing control characters, cue widths outside 16–60,
zero or more than 16 panes, duplicate or blank pane IDs, empty shells, weights
outside 1–1000, missing/blank cue names, missing/empty command lists, repeated pane targets within a cue, unknown
command targets, invalid layout trees (including missing/duplicate/unknown pane references), and blank commands or commands containing NUL/CR/LF. Integer
bounds are inclusive. A disabled sidebar still requires a valid `cue_width`.

Validation does not execute anything or check shell syntax, executable
availability, working-directory existence, service readiness, or whether a
terminal forwards a chosen key. Shell and cwd failures appear when sessions
start. There is no per-command `delay`, `timeout`, `env`, `cwd`, or completion
condition; unsupported keys are rejected rather than ignored.

Full-demo apply resets queue progress. Panes with matching ID, shell, arguments,
and working directory retain their sessions. A changed identity or shell setup
starts a replacement session, and removed panes are closed. Pane title, scheme, and weight
changes reuse the shell. Title, prefix, and sidebar changes also preserve matching
sessions. A cue-only edit rewinds the edited item if it is the current playback item;
editing another item leaves playback progress unchanged.
See [editing and saving](../guide/editor.md) for the save/load workflow.

## Editing a whole demo versus one cue

**Prefix, o**, or plain **o** in the cue list, edits a full document matching this
reference. **e** in the cue list edits only one cue: `name`, `description`, and
`commands`, without a `[[queues]]` heading. The optional cue `layout` is valid there. Top-level demo `title` and `panes`
definitions are not valid in that single-cue editor.

Use Ctrl-L/F2 in the full editor to load an entire file, Ctrl-G/F4 to apply, or
Ctrl-S/F3 to save and apply. Loading only previews; applying never executes queued
commands. Saving normalizes TOML and drops comments; runtime queue progress is not
saved. See [editing and saving](../guide/editor.md) for a step-by-step workflow.

## Global and cue-specific layouts

The top-level `layout` is the initial arrangement. A cue can provide its own
preset or tree (including weights) without redefining panes:

```toml
[[queues]]
name = "Focus the service"
layout = { direction = "columns", children = [
  { pane = "service", weight = 2 },
  { direction = "rows", children = [{ pane = "observer" }, { pane = "notes" }] },
] }
commands = [{ pane = "service", command = "pwd" }]
```

All three pane IDs must exist in `[[panes]]`. The override applies when the first
non-skipped command of that cue is dispatched, or when `t` types its commands.
Preview, selection, and skip do not apply it. Omission keeps the active layout;
there is no implicit reset on each cue. Full-demo apply/reload restores the
global layout. To restore a specific arrangement during playback, explicitly
repeat that layout in a later cue. Replaying a cue reapplies its override.

Cue trees may show a subset of configured panes, including a single pane with
`layout = { pane = "service" }`. Omitted panes stay alive at their last terminal
size and continue collecting output and scrollback. Later layouts can reveal
them without restarting their shells. Focus navigation skips hidden panes.
Preset layouts always include all configured panes.

Resizing or cycling the layout while a cue override is active edits that cue's
layout in memory; without an override, it edits the global layout. Save the full
demo to persist either. Node weights provide per-cue sizing; pane-table weights
remain shared defaults for the preset layouts. Adding an ad hoc pane extends all
explicit trees, including layouts in future cues. Removing a pane manually
requires removing it from every tree and its command targets.

## Key actions, timers, and native clear

```toml
loop = true # Top level, before any tables.

[[panes]]
id = "worker"

[[queues]]
name = "Interrupt the foreground process"
advance_after_ms = 1500
commands = [
  { pane = "worker", keys = [{ key = "Ctrl+C" }, { key = "<esc>", repeat = 2 }] },
]

[[queues]]
name = "Clear the display"
advance_after_ms = 3000
commands = [{ pane = "worker", clear = true }]
```

Each key table accepts only:

| Field | Type | Default | Validation |
| --- | --- | --- | --- |
| `key` | String | Required | A portable terminal key name or a literal Unicode character. |
| `repeat` | Integer | `1` | 1–4096; all repeats in one pane entry together must total at most 4096. |

Names are case-insensitive: `Esc`/`Escape`, `Enter`/`Return`, `Tab`, `BackTab`,
`Backspace`, `Space`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`,
`PageDown`, `Insert`, `Delete`, and `F1`–`F12`. Optional enclosing angle brackets
are accepted (`<esc>`). Literal characters retain case, including Unicode and `+`.
Prefix a name with `Ctrl+`/`Control+`, `Alt+`/`Option+`, or `Shift+`;
combine them for cursor, navigation, and function keys. `Shift+Tab` is BackTab.
For characters, use the actual uppercase character/symbol instead of Shift.
Ctrl characters accept ASCII letters, Space, `@`, `[`, `\`, `]`, `^`, `_`, `?`.
Enter and Backspace support Alt only; Esc, Tab, and BackTab have no other
modifier combinations. Command/Super and unsupported combinations are rejected
by `--check`; these are terminal byte sequences, not operating-system key events.

`keys = [{ key = "Ctrl+D" }]` sends EOF to a shell at an empty prompt and may
close it. Keys go directly to the pane, bypassing nysos shortcuts. Arrow encoding
honors the target application's cursor mode. Repeats are contiguous input, with
no delay or completion detection between presses. Application-specific keymaps
still determine their effect.

Preview labels key actions as **Send keys**, includes every repeat count, and
labels native clears separately. `t` immediately sends key and clear actions;
for cues containing only these actions it advances exactly like Enter. In a
mixed cue, `t` types command text without Enter, sends key/clear actions, focuses
the first target shell, and leaves playback unchanged. Replaying that cue with
Enter will send every action again; finish the typed commands in their shells
and skip the cue's entries if repetition is unwanted.

A timer starts after the last action is dispatched, including a prefix-n step
that completes a cue. It executes the next cue without waiting for process
completion. Skipping the final entry does not arm a timer. Plain startup does
not start playback; run a cue first. A cue without `advance_after_ms` stops the
automatic chain. Without looping, the final cue stops even if it has a timer;
Enter can still replay it. With looping, the final cue selects the first, and its
timer can execute that first cue again. A one-cue loop runs at most once per UI
frame (approximately 16 ms); timers are best-effort, not real-time scheduling.

Space in the cue list pauses/resumes an armed timer. Preview, help, and editor
modals suspend its countdown. Browsing/selecting a cue or restoring a bookmark
cancels automatic playback. Applying/editing configuration cancels it too. Errors
stop automatic playback and appear in the status line. Focus changes alone do
not pause playback; the countdown remains visible even with the header hidden.

Native clear clears the emulator viewport and preserves normal scrollback and
cursor position. It sends no command, keystroke, or signal to the process;
it does not reset shell state or erase a partially typed line. A running program
can redraw afterward. Alternate-screen programs do not provide normal history.

Before dispatch, nysos records the cue's effective layout and the execution lines
of visible panes and action targets. Cue-list `s` restores that layout and returns
those panes to the most recent recorded start of the selected cue. Recording also
happens with `t`, including when a cue inherits its layout from an earlier cue.
You can browse recorded cues backward or forward without overwriting the live
layout or changing playback progress. `b` restores the current live layout,
including any manual resizing, and returns **all** panes to the live bottom.
Dispatching or typing a cue also leaves the historical view first, so cues without
an override inherit the live layout. Resizing/cycling a historical view changes
only that temporary view; it does not change saved snapshots or the live demo.
Unexecuted cues have no snapshot and leave the view unchanged.
Bookmarks survive pane resizing, layout changes, gutter toggles, and text reflow.
They are in-memory and expire on alternate-screen transitions, history deletion, pane replacement, or configuration edits. They
also expire when their lines are evicted from the terminal’s 10,000-line history.
Recent bookmarks continue working when history is full. Missing bookmarks do not
restore a scroll position and are reported in the status line; the recorded layout
can still be restored. Layout snapshots expire on configuration edits, including
adding an ad hoc pane. A line still on the live screen may not yet be
scrollable to the top. These controls do not undo commands or rewind processes.

See [`examples/key-playback.toml`](https://github.com/McCodeman/nysos/blob/main/examples/key-playback.toml)
for a timed loop that interrupts a process, prints new output, and clears a pane.

## Gated line-number gutters

See [feature gates and line numbers](../guide/feature-gates.md) for precedence,
live toggling with **prefix, #**, logical numbering, scrolling, mouse behavior,
and experimental limits. Visibility settings are ignored when the gate is off.
Saving after a keyboard toggle persists the focused pane's initial setting;
per-cue overrides remain in their actions and may change it again during playback.
