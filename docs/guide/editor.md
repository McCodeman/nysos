# Editing and saving

**Ctrl-G, o** (your configured prefix, then `o`) opens the **entire demo**,
including its title, every cue, and all panes. In the cue sidebar, plain **o**
opens the same editor; **e** edits only the selected cue.

The full demo editor is a multiline TOML editor with cursor movement, selection,
copy/paste inside the editor, and undo/redo provided by tui-textarea. Paste from
the system clipboard with your host terminal's paste shortcut.

| Shortcut | Action |
| --- | --- |
| Ctrl-L / F2 | Enter a path to load; Enter previews the file |
| Ctrl-S / F3 | Validate, enter a save path, then Enter saves and applies |
| Ctrl-G / F4 | Validate and apply without saving |
| Esc | Cancel the popup |

Use the **Control** key on macOS, not Command (⌘). Inside the editor, press the
shortcut directly: do **not** send the nysos prefix first. For example, Ctrl-S
opens Save; Ctrl-G applies immediately rather than starting a command sequence.
The F2/F3/F4 alternatives may require Fn on a Mac keyboard.

Save and Apply validate TOML before proceeding. If the editor stays open, read
the error below the text: fix it and press the action again. Save first opens a
path prompt; press Enter there to finish saving. Errors stay in the popup so you
can correct them. Save replaces an existing
file atomically on macOS/Linux. Paths are literal, relative to the launch
directory; use an absolute path rather than `~` expansion. Esc in a path prompt
cancels the editing operation. Saved files are normalized TOML; comments are not
preserved.

Applying a full demo resets queue progress. Existing sessions are retained when
ID, shell, arguments, and working directory match. Changing those fields
starts replacement sessions; removing panes closes them. Editing only
titles, colors, layout weights, or queues does not restart matching shells.
`id` is a pane's stable identity; `title` is its display label. Legacy `name`
is accepted as an alias for `id`. Use the modal to
save ad hoc panes, resized layouts, and edited queues for your next presentation.

Press **e** in the cue list to edit the selected item. **Prefix, e** edits that
selection while the list is focused, or the current playback item from a shell.
Apply executes nothing. Editing the current playback item rewinds it to its
first command; editing another item preserves playback progress. Esc cancels.
Use the full demo editor to save edited cues, sidebar visibility, and width.


## Add cues and rename the demo

1. Click the cue list and press **o**, or press **Ctrl-G**, release it, then **o**.
2. To load a file from disk, press **Ctrl-L** (or **F2**), enter its path, and
   press **Enter**. This previews the entire file. Opening the editor without
   loading starts with the current in-memory demo, including unsaved changes.
3. Edit the top-level `title = "My demo"` before the first table heading.
4. Add a new `[[queues]]` block at the end, or edit/reorder existing blocks:

   ```toml
   [[queues]]
   name = "Inspect the service"
   description = "Show the current directory and files."
   commands = [
     { pane = "presenter", command = "pwd" },
     { pane = "observer", command = "ls -lah" },
   ]
   ```

   Use an existing pane ID in each command. Each cue needs at least one
   command and may target each pane only once. To remove a cue, remove its whole `[[queues]]` block.
5. Press **Ctrl-S** (or **F3**), choose a save path, and press **Enter** to save
   and apply. **Ctrl-G** (or **F4**) applies without saving. **Esc** cancels.

The demo title appears above the current cue when the header is enabled.
Applying the full demo resets cue progress and executes no queued commands.
Matching shell sessions are preserved.

## A typical edit

1. Press **prefix, o** to open the full demo as TOML.
2. Edit `layout`, a pane's `scheme` or `weight`, or the command list.
3. Press **Ctrl-G** to apply in memory, or **Ctrl-S** to choose a save path.
4. In the save prompt, edit the path with the arrow and Backspace keys, then
   press **Enter**. An existing file at that path is replaced.

Loading with **Ctrl-L** previews a file in the editor; it does not start its
shells or execute commands. Review it, then apply or save it. New and replacement
shells start only when the configuration is applied. Saving writes the file
before applying it; if a shell fails to start, the file is saved but the running
demo remains unchanged. Correct the shell path in that file and load it again.

Validation errors keep the editor open. A failed apply also preserves the
running demo. Esc cancels the entire editing operation, including from the
load/save path prompt. Unsaved edits are discarded. Shells continue running
behind the modal.

Use the [configuration reference](../reference/configuration.md) for allowed
fields and values. A queue-only editor accepts a single `name`, `description`,
and `commands` table without a `[[queues]]` heading.

## Diagnose keys in your actual session

Run a freshly built nysos with `--debug-keys` (for example,
`nix develop --command cargo run -- --debug-keys --config demo.toml`). The bottom
line shows the last key event and the current editor mode, even while a popup is
open. It displays input on screen only; it does not write a key log. Avoid typing
sensitive input while this diagnostic is enabled.

Open the full editor, then press Ctrl-L once. Expected input is `Char('l')`,
`CONTROL`, `Press`; the mode should change from `full demo` to `load path`.
Ctrl-S similarly changes to `save path` when the TOML is valid. Ctrl-G closes the
editor after a successful apply. If the last event does not change, the key did
not reach nysos. `SUPER` indicates a Command-key event, and `Paste` is not treated
as a shortcut. `single cue` means the full-file Load/Save actions are unavailable;
close it with Esc and open the full demo with prefix-o.

If a correctly received key leaves the full editor open, read the validation
error below the text. Include the displayed input, mode, error (if any), and
whether you are using tmux or SSH when reporting the problem. Exit and relaunch
without `--debug-keys` to hide the diagnostic line.

## Change pane titles and themes per cue

Run `nysos --config examples/pane-transitions.toml` from the source checkout for
a six-cue example. It updates each pane independently, then updates both together;
it also demonstrates title-only and theme-only overrides alongside new commands.

In the cue editor (`e`), add `title` and/or `scheme` to a command:

```toml
name = "Inspect logs"
commands = [
  { pane = "presenter", command = "tail -20 app.log", title = "Server logs", scheme = "forest" },
]
```

`pane` always refers to the stable ID from `[[panes]]`. The title and theme change
when you run or type the command, and persist until another command changes them.
Preview and skip leave the appearance unchanged. To set the initial title, edit
`title` under `[[panes]]` in the full demo editor. Saving retains those initial
values plus the cue overrides; applying the full demo restores its initial appearance.
