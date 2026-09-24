<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pane layouts and resizing

A demo can combine columns, rows, and nested groups. Each leaf identifies a
pane by its stable `id`; commands keep targeting that ID when you rearrange it.
The cue sidebar is separate from this layout and stays on the far left.

The built-in demo (`nysos --demo`) starts with three shell panes: Service fills the left
column, with Observer above Notes in the right column. `nysos --init demo.toml`
exports that layout and its twelve cues.

For a more deeply nested five-pane example, run from the repository root:

```sh
nysos --config examples/nested-layout.toml
```

Its layout looks like this (excluding the cue sidebar):

```text
+---------------------------+-------------+
| Presenter                 | Logs        |
+-------------+-------------+             |
| Build       | Tests       +-------------+
|             |             | Notes       |
+-------------+-------------+-------------+
```

The left column has twice the width of the right. It contains two rows, and its
lower row contains two columns. The right column contains two rows.

Put this layout before the pane/queue definitions in your TOML:

```toml
[layout]
direction = "columns"
children = [
  { direction = "rows", weight = 2, children = [
    { pane = "presenter" },
    { direction = "columns", children = [{ pane = "build" }, { pane = "tests" }] },
  ] },
  { direction = "rows", children = [{ pane = "logs" }, { pane = "notes" }] },
]
```

Define all five IDs with `[[panes]]` tables. Every configured pane must appear
exactly once in the initial tree. Cue trees may omit panes to hide them.
The [configuration reference](../reference/configuration.md#nested-layout-trees)
lists all fields and validation rules. Existing `layout = "columns"`, `"rows"`,
and `"grid"` files continue to work.

## Edit, save, and add panes

Press **Ctrl-G, o** to edit the full demo, including `[layout]` and `[[panes]]`.
Use **Ctrl-G/F4** in the editor to apply or **Ctrl-S/F3** to save and apply.
Changing the layout preserves matching shell sessions and their contents.
A cue can override `layout` with the same tree schema and its own weights.
It applies on execution or `t`, and persists until another cue override or a
full-demo apply. Previewing/skipping does not apply it. Mouse and keyboard
resizing change the active cue's tree when an override is active, otherwise the
global layout. Saving persists both. The built-in demo changes layouts only at cues 5, 8, and 10: two visible panes,
then one, then all three again. Several command cues inherit each arrangement,
showing that a command can target one or several panes without moving them.

A cue such as `layout = { pane = "service" }` shows only Service. Omitted panes
keep their shells, terminal size, and scrollback, and continue collecting output.
Focus navigation skips hidden panes. Later layouts can show them again.

Each executed or typed cue records its effective layout, including inherited
layouts and current weights. In Cues, **s** restores that layout and its pane
bookmarks. You can browse recorded cues in either direction without changing
playback progress or the current live layout. **b** restores the live layout,
including manual resizing, and scrolls all panes to bottom. Resizing or cycling
a historical view affects only that temporary view; saving still saves the live
configuration. Dispatching or typing a cue returns to the live layout first.

**Ctrl-G, a** adds an ad hoc pane at the right of a custom layout. A root column
split gets another child with the average weight of its siblings; any other root
is wrapped in a new two-column split with equal weights. The existing arrangement
stays intact inside its group. All explicit cue trees are extended too so future
cues still include it. Edit the trees to move the new pane elsewhere.
Keyboard focus order and pane numbers follow `[[panes]]` order, not tree order.
**Ctrl-G, l** replaces a custom layout with the columns preset, then cycles
rows/grid/columns. Save first if you want to retain the custom tree.

## Font size and window size

Change font size with your terminal's own zoom controls. For example, Ghostty
provides [increase/decrease/reset font-size actions](https://ghostty.org/docs/config/keybind/reference#increase_font_size).
Nysos reads the resulting terminal cell dimensions and recomputes every nested
split from the same weights. Each child PTY is resized so shells and full-screen
programs see their new row/column counts. No shell restart is needed.

Nysos does not set fonts or work in pixel units. If the font changes but the
reported cell dimensions stay the same, the layout in cells stays the same.
Integer-cell rounding is expected. The header, footer, cue width, and pane borders
use cells rather than weighted proportions. Very small windows can collapse pane
content; enlarging the window restores it without changing stored weights.
Through a multiplexer, its allocated pane size is authoritative. Over SSH, use a
PTY (`ssh -t`); a client that does not forward window-size changes cannot provide
accurate dimensions to the remote app.

## Dragging borders

Drag either edge of the two-cell border between adjacent panes or groups with
the **left mouse button**, without modifiers. An outer divider resizes the whole
nested group. An inner divider changes only its adjacent siblings. At a crossing,
the deepest split takes priority; drag farther along the outer border to select
that group instead. Outer window borders are not resize handles.

Drag bounds leave at least four cells per adjacent group when there is enough
space. This is a group limit, not a guarantee that every descendant pane has a
readable minimum size. Tiny groups refuse further drag resizing. Terminal-size
changes, key presses, mouse release, or focus loss cancel an active drag so stale
coordinates cannot change a newly arranged screen.

Mouse resizing requires press, motion, and release reports from the terminal.
Those are terminal escape sequences; SSH can carry them without a graphical
session, but every terminal/multiplexer in the path must forward them.

| Environment | Feasibility and limitations |
| --- | --- |
| Ghostty directly | Supports application mouse reporting. Use an unmodified left-button drag. `mouse-reporting = false` disables forwarding; Shift commonly selects text instead. See [Ghostty mouse options](https://ghostty.org/docs/config/reference#mouse-reporting). |
| tmux | Enable `tmux set-option -g mouse on` and leave copy mode. Tmux retains its own borders/status bar; drag inside nysos. Custom mouse bindings can intercept events. See [tmux mouse support](https://github.com/tmux/tmux/wiki/Getting-Started#using-the-mouse). |
| SSH with a PTY | Works when the local terminal reports mouse events and intermediaries forward them. Network latency can make dragging lag; keyboard adjustments are often easier. `ssh -t` provides a PTY, but cannot add mouse support to a terminal that lacks it. See [OpenSSH](https://man.openbsd.org/ssh.1). |
| Nested multiplexers, web consoles, older terminals | May intercept gestures, support clicks without motion, or discard reports. A terminal name/TERM value alone cannot reliably establish support. Use keyboard resizing if dragging does nothing. |

Nysos requests mouse reporting but cannot reliably detect events silently consumed
upstream. It does not wait for a mouse-capability handshake or disable keyboard
controls when mouse reports are unavailable. A click without motion only changes
focus. A drag without a preceding border press does not resize anything.

## Keyboard fallback

Focus a shell with **Ctrl-G, Tab**, then press/release **Ctrl-G** before each
adjustment:

| Key | Action |
| --- | --- |
| `<` | Make the focused pane/group about two columns narrower |
| `>` | Make it about two columns wider |
| `-` | Make it about one row shorter |
| `+` | Make it about one row taller |

These commands resize the nearest containing split on the requested axis. If the
pane spans that axis inside a larger group, the containing group is resized.
They report when there is no applicable split or the cue list is focused.
The `grid` preset keeps equal row heights; switch to a nested layout to control
those heights. Ratios and cell rounding can limit one-step adjustments.

To keep all mouse gestures with the host terminal, run:

```sh
nysos --no-mouse --config examples/nested-layout.toml
```

This disables capture for the entire nysos session, including its child apps.
Focus, cue navigation, preview, editing, and resizing remain available from the
keyboard. Saved TOML weights also provide an exact, mouse-independent way to
set proportions.
