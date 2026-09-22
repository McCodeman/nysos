# Demo configuration reference

A demo is a UTF-8 TOML file selected with `nysos --config PATH` or loaded through
the modal. There is no automatic discovery of `demo.toml`. Unknown fields and
invalid enum values are rejected. All paths resolve relative to nysos's launch
directory, not the configuration file's directory; `~` and environment variables
inside configured paths are not expanded.

## Complete example

This example uses `/bin/sh` on both macOS and Linux:

```toml
title = "Service walkthrough"
prefix = "ctrl-g"
header = true
cue_list = true
cue_width = 26
layout = "columns"

[[panes]]
name = "presenter"
shell = "/bin/sh"
args = []
scheme = "ocean"
weight = 3

[[panes]]
name = "observer"
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
  { pane = "presenter", command = "ls -lah" },
  { pane = "observer", command = "printf 'https://ratatui.rs\\n'" },
]
```

## Top-level fields

| Field | Type | Default when omitted | Meaning |
| --- | --- | --- | --- |
| `title` | String | `"nysos demo"` | Nonempty demo title displayed above cue details when the header is enabled; control characters are rejected. |
| `terminal_keys` | String enum | `"auto"` | `"auto"` detects Ghostty using TERM_PROGRAM/TERM; `"ghostty"` adds Alt-B/F focus aliases; `"standard"` preserves those shell keys. `--terminal-keys` overrides at startup. |
| `prefix` | String enum | `"ctrl-g"` | Control prefix: `"ctrl-g"`, `"ctrl-a"`, `"ctrl-b"`, or `"f12"`. `--prefix` overrides it at startup. |
| `header` | Boolean | `true` | Show queue item index, name, description, and next command above the panes. |
| `cue_list` | Boolean | `true` | Show the left cue sidebar; toggle with Ctrl-G, c. |
| `cue_width` | Integer | `26` | Sidebar width including borders, 16–60 columns; capped at half the content width in small terminals. |
| `layout` | String enum | `"columns"` | `"columns"`, `"rows"`, or `"grid"`. |
| `panes` | Array of pane tables | Built-in `presenter` and `observer` panes | Between 1 and 16 pane definitions; their count determines pane count. |
| `queues` | Array of queue tables | Built-in `Welcome` item | Ordered queue items; an empty array is allowed. |

Defaults apply independently: omitting `queues` retains the built-in commands
for `presenter` and `observer`. If you define other pane names, explicitly supply
`queues` or set `queues = []`. Otherwise validation may report an unknown target.
An empty TOML file describes the built-in demo; `panes = []` is invalid.

For a purely interactive single-pane demo:

```toml
queues = []

[[panes]]
name = "scratch"
```

Put top-level assignments before the first `[[panes]]` or `[[queues]]` table;
TOML assignments after a table header belong to that table.

## Pane fields: `[[panes]]`

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `name` | String | `"shell"` | Nonempty, unique, case-sensitive identity; also used as the visible title. |
| `shell` | String | `$SHELL`, or `"/bin/sh"` if unavailable | Nonempty executable path or name. Use `args` for arguments. |
| `args` | Array of strings | `[]` | Arguments passed directly to the shell executable, e.g. `["-l"]`. |
| `cwd` | String | Inherit launch directory | Working directory for this pane. Omit for inheritance; TOML has no null value. |
| `scheme` | String enum | `"ocean"` | `"ocean"`, `"ember"`, `"forest"`, or `"mono"`. |
| `weight` | Integer | `1` | Relative size from 1 through 1000. |

Give every pane an explicit name: two panes that omit `name` both become `shell`
and fail validation. Names are matched exactly, without trimming or case folding.
A name that contains only whitespace is rejected.

Shell paths and working directories are checked only when a PTY starts, not by
`--check`. Shell commands do not belong in `shell`: use `shell = "/bin/zsh"` and
`args = ["-l"]`, rather than `shell = "/bin/zsh -l"`. Every PTY receives
`TERM=xterm-256color` and `COLORTERM=truecolor`.

### Layout and weights

- `columns`: panes appear side by side; weights control widths.
- `rows`: panes appear top to bottom; weights control heights.
- `grid`: panes fill rows in configuration order with `ceil(sqrt(pane_count))`
  columns. Rows have equal height, and weights control widths within each row.
  A partially filled last row still uses its full width.

Weights are ratios, not percentages or fixed sizes. Two column panes weighted
3 and 2 occupy approximately 60% and 40% of the available width, including their
borders. Dragging a divider updates weights; save the demo to persist them.

### Color schemes

| Scheme | Appearance |
| --- | --- |
| `ocean` | Cool foreground, dark blue background, cyan focus border |
| `ember` | Warm foreground, dark red background, orange focus border |
| `forest` | Pale green foreground, dark green background, green focus border |
| `mono` | Gray foreground, near-black background, white focus border |

Schemes control default pane colors and the focus border. Programs' ANSI colors
are rendered separately. Custom RGB theme definitions are not supported yet.

## Queue fields: `[[queues]]`

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `name` | String | Required | Nonempty label in the header; queue names need not be unique. |
| `description` | String | `""` | Presenter-facing description in the header. |
| `commands` | Array of command tables | Required | One or more commands, sent in listed order. |

Ctrl-G, n sends a single command. Enter in the cue sidebar sends all remaining
commands of the selected item in order, without waiting for completion. Commands
can target different panes within the same item. Skipping advances
past one command; finishing the last command advances to the next queue item.

## Command fields

| Field | Type | Default when omitted | Meaning / validation |
| --- | --- | --- | --- |
| `pane` | String | Required | Exact name of an existing pane. |
| `command` | String | Required | Nonempty single-line shell input; NUL, CR, and LF are rejected. |

There is no per-command delay, environment, timeout, working directory, or
completion condition. Commands run in the target pane's existing shell state.
Use shell separators for compound commands or invoke a script for multiline work.

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

## Validation and applying changes

```sh
nysos --config demo.toml --check
```

Validation rejects malformed TOML, unknown fields, zero or more than 16 panes,
duplicate or blank pane names, empty shells, out-of-range weights, empty queue
names/lists, unknown command targets, and blank or multiline commands. It does
not execute anything or check shell syntax.

Full-demo apply resets queue progress. Panes with matching name, shell, arguments,
and working directory retain their sessions. A changed identity or shell setup
starts a replacement session, and removed panes are closed. Scheme and weight
changes reuse the shell. A cue-only edit rewinds the edited item if it is the current playback item;
editing another item leaves playback progress unchanged.
See [editing and saving](../guide/editor.md) for the save/load workflow.
