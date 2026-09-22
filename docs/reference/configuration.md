# Demo configuration reference

A demo is a UTF-8 TOML file selected with `nysos --config PATH` or loaded through
the modal. Plain `nysos` starts two interactive panes with no cues; `nysos --demo`
explicitly loads the built-in six-cue example. There is no automatic discovery of `demo.toml`. Unknown fields and
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
| Top level | `title`, `prefix`, `terminal_keys`, `header`, `cue_list`, `cue_width`, `layout`, `panes`, `queues` |
| Each `[[panes]]` | `id`, `name` (legacy alias for `id`), `title`, `shell`, `args`, `cwd`, `scheme`, `weight` |
| Each `[[queues]]` | `name`, `description`, `commands` |
| Each command in a queue | `pane`, `command`, `title`, `scheme` |

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
| `title` | String | `"nysos demo"` | Nonempty demo title displayed above cue details when the header is enabled; control characters are rejected. |
| `terminal_keys` | String enum | `"auto"` | `"auto"` detects Ghostty using TERM_PROGRAM/TERM; `"ghostty"` adds Alt-B/F focus aliases; `"standard"` preserves those shell keys. `--terminal-keys` overrides at startup. |
| `prefix` | String enum | `"ctrl-g"` | Control prefix: `"ctrl-g"`, `"ctrl-a"`, `"ctrl-b"`, or `"f12"`. `--prefix` overrides it at startup. |
| `header` | Boolean | `true` | Show the demo title, cue index/name/description, and command index above the panes. When the cue list is focused, show its selection. |
| `cue_list` | Boolean | `true` | Show the left cue sidebar; toggle with Ctrl-G, c. |
| `cue_width` | Integer | `26` | Sidebar width including borders, 16–60 columns; capped at half the content width in small terminals. |
| `layout` | String enum | `"columns"` | `"columns"`, `"rows"`, or `"grid"`. |
| `panes` | Array of pane tables | Built-in `presenter` and `observer` panes | Between 1 and 16 pane definitions; their count determines pane count. |
| `queues` | Array of queue tables | `[]` | Ordered queue items; an empty array is allowed. |

Omitting `queues` gives an empty cue list, including when defining custom panes.
An empty TOML file describes two interactive shells with no cues; `panes = []`
is invalid. No sample commands are inserted into files that omit `queues`.

The omitted `panes` array creates `presenter` (ocean) and `observer` (ember),
each with weight 1 and the normal shell/args/cwd defaults.
`nysos --demo` explicitly loads the six-cue pane transitions example, using
`service` and `observer` panes with `/bin/sh`. It demonstrates independent pane
updates, updates to both panes, title-only overrides, and theme-only overrides.
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
current values. Other configuration fields have no corresponding CLI overrides.
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
| `weight` | Integer | `1` | Relative size from 1 through 1000. |

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
| `name` | String | Required | Label in the cue list and header; cannot be empty or whitespace-only. Cue names need not be unique. |
| `description` | String | `""` | Free-form presenter-facing text in the header; empty or multiline text is accepted, but the fixed-height header can clip long text. |
| `commands` | Array of command tables | Required | One or more commands, sent in listed order; each pane may appear at most once per cue. |

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
| `command` | String | Required | Nonempty single-line shell input; NUL, CR, and LF are rejected. |
| `title` | String | Keep current pane title | Change the target pane’s display title when dispatched; nonempty with no control characters. |
| `scheme` | String enum | Keep current pane scheme | Change the target pane’s theme: `"ocean"`, `"ember"`, `"forest"`, or `"mono"`. |

Title and scheme overrides apply when the command is sent (Enter in the cue list
or Prefix, n), or typed without execution using `t`. They affect only the target
pane and preserve its ID, shell, and scrollback. Subsequent cues keep the current
values unless they explicitly override them. Browsing, previewing, and skipping
commands do not apply styling. If jumping between cues, specify both fields when
a cue needs a particular appearance regardless of the previously executed cue.
Commands remain required; these overrides do not create styling-only commands.

Runtime overrides are not written back into the initial `[[panes]]` definitions.
Saving preserves the initial pane configuration and the overrides in each cue.
Applying a full demo restores its initial titles and schemes while retaining
matching shell sessions. Restarting a pane retains its current appearance.

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
command targets, and blank commands or commands containing NUL/CR/LF. Integer
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
`commands`, without a `[[queues]]` heading. Top-level fields such as `title` and
pane settings are not valid in that single-cue editor.

Use Ctrl-L/F2 in the full editor to load an entire file, Ctrl-G/F4 to apply, or
Ctrl-S/F3 to save and apply. Loading only previews; applying never executes queued
commands. Saving normalizes TOML and drops comments; runtime queue progress is not
saved. See [editing and saving](../guide/editor.md) for a step-by-step workflow.
