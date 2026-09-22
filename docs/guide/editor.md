# Editing and saving

**Prefix, o** opens a multiline TOML editor with cursor movement, selection,
copy/paste inside the editor, and undo/redo provided by tui-textarea. Paste from
the system clipboard with your host terminal's paste shortcut.

| Shortcut | Action |
| --- | --- |
| Ctrl-L / F2 | Enter a path to load; Enter previews the file |
| Ctrl-S / F3 | Validate, enter a save path, then Enter saves and applies |
| Ctrl-G / F4 | Validate and apply without saving |
| Esc | Cancel the popup |

Errors stay in the popup so you can correct them. Save replaces an existing
file atomically on macOS/Linux. Paths are literal, relative to the launch
directory; use an absolute path rather than `~` expansion. Esc in a path prompt
cancels the editing operation. Saved files are normalized TOML; comments are not
preserved.

Applying a full demo resets queue progress. Existing sessions are retained when
name, shell, arguments, and working directory match. Changing those fields
starts replacement sessions; removing panes closes them. Editing only
colors, layout weights, or queues does not restart matching shells. Since `name`
is also a pane's identity, changing a name starts a new shell. Use the modal to
save ad hoc panes, resized layouts, and edited queues for your next presentation.

Press **e** in the cue list to edit the selected item. **Prefix, e** edits that
selection while the list is focused, or the current playback item from a shell.
Apply executes nothing. Editing the current playback item rewinds it to its
first command; editing another item preserves playback progress. Esc cancels.
Use the full demo editor to save edited cues, sidebar visibility, and width.


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
