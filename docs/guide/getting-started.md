<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Getting started

Install nysos, open the included three-pane demo, then rehearse previewing,
executing, and editing its six cues. You do not need the Alacritty application.

## Install

=== "Homebrew"

    On macOS or Linux with [Homebrew](https://brew.sh/) installed:

    ```sh
    brew install mccodeman/tap/nysos
    nysos --version-full
    ```

    Update an existing installation with `brew update && brew upgrade nysos`.
    Homebrew builds from the published release and installs `man nysos`.
    If that release predates a feature described here, use the source tab to try
    the current checkout. In particular, the historical v0.1.1 release predates
    nested layouts and the three-pane demo.

=== "Clone and install from source"

    Install [Git](https://git-scm.com/downloads) and
    [Nix with flakes enabled](https://nixos.org/download/), then:

    ```sh
    git clone https://github.com/McCodeman/nysos.git
    cd nysos
    nix develop --command make install
    export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    nysos --add-to-path --install-completions
    nysos --version-full
    ```

    This installs the compiled command under `${CARGO_HOME:-$HOME/.cargo}/bin`.
    The setup command makes it take precedence over Homebrew in new Bash/Zsh
    shells. Reopen the terminal to load completions. After changing source,
    repeat `nix develop --command make install`; no Homebrew update is needed.

For Debian/Ubuntu packages, Fedora/RHEL-family packages, or a download-and-install
script, see [installation and packages](installation.md). Check the installed
command with `command -v nysos` if you have multiple installations.

## Open the demo

```sh
nysos --demo
```

You should see a cue list at the far left and three shell panes:

```text
Cue list    +------------------+------------------+
            |                  | Observer         |
            | Service          +------------------+
            |                  | Notes            |
            +------------------+------------------+
```

The header describes the selected cue. Titles use the theme's high-contrast
foreground, including when a pane is not focused. No commands run until you ask.
Plain `nysos` starts two interactive shells with no demo cues; `--demo` is explicit.

All **Ctrl-G, key** instructions mean: press Ctrl-G, release it, then press the
second key. Use Control, not Command, on macOS. Ctrl-G avoids tmux's Ctrl-B prefix.

## Preview and execute your first cue

1. Press **Ctrl-G, 0** to focus the cue list. **Up/Down** selects a cue.
2. Select **Introduce all three shells** and press **p**. Each pane shows an
   overlay containing its next command. Nothing has executed.
3. Press **Esc** to close the overlays and restore the shell panes.
4. Press **Enter** while the cue list is focused. All three commands are sent,
   titles/themes change, and selection advances to **Service only**.
5. Wait for the shell prompts. Press **p**, **Esc**, then **Enter** again. This
   cue widens the Service column and runs only its command.
6. Execute **Observer only** next. Its cue layout gives Observer more height
   above Notes. Other shell sessions remain intact.

Enter sends the selected cue's remaining commands without waiting for them to
finish. To step one command at a time instead, use **Ctrl-G, n**. **Ctrl-G, s**
skips the next command. Skipping and previewing do not change titles, themes,
or cue layouts. The final cue restores balanced columns/rows. Pressing Enter
again on the final cue executes it again; replay is intentional and unlimited.

## Move between panes and work interactively

Press **Ctrl-G, 1** to focus Service, **Ctrl-G, 2** for Observer, or **Ctrl-G, 3**
for Notes. Type `pwd` and press Enter in any shell. Its history and variables
persist independently of the others.

**Ctrl-G, Tab** cycles through panes and the cue list. Click-to-focus works when
mouse events are forwarded. Ghostty's default Option-Left/Right also rotates
focus; use the prefix shortcuts if your terminal intercepts those keys.
**Ctrl-G, 0** always returns to the cue list; **Ctrl-G, c** hides or shows it.

## Edit a cue before sending it

1. Return to the cue list and select **Title only** with **Up/Down**.
2. Press **e**. This edits one cue's TOML, including its name, description,
   commands, optional title/theme overrides, and optional layout.
3. Change the text inside a quoted `printf` command, preserving valid TOML.
4. Press **Ctrl-G** directly inside the editor, or **F4**, to apply. There is
   no prefix sequence inside a modal. Invalid TOML leaves the editor open with
   an error; **Esc** cancels without applying.
5. Press **p** to inspect the revised command, **Esc** to close, and **Enter**
   to execute it.

To edit shell input instead of TOML, select a cue and press **t**. Its next command
per target pane is typed without Enter and the first target shell gets focus.
Edit the line using normal shell keys, then press Enter yourself. For a cue with
multiple targets, focus each target and confirm its line separately. Typing does
not advance cue progress: running that cue from the list later sends it again.

## Resize and add a pane

Drag a shared border with the left mouse button. The outer vertical divider
resizes both columns; the horizontal divider on the right divides Observer and
Notes. Font zoom and window resizing recompute proportions from the terminal's
reported row/column count.

Without a mouse, focus a shell and use **Ctrl-G, <** / **Ctrl-G, >** for width,
or **Ctrl-G, -** / **Ctrl-G, +** for height. Each adjustment acts on the nearest
containing split on that axis. `nysos --no-mouse --demo` disables capture while
retaining all keyboard controls. See [layouts and resizing](layouts.md) for
terminal, tmux, and SSH limitations.

Press **Ctrl-G, a** to add a shell for an ad hoc command. It appears at the right
of the demo and receives focus. Panes can be renamed, rethemed, and rearranged
in the full editor. Adding one updates global and cue layout trees to include it.

## Save your own demo

1. Press **Ctrl-G, o**, or **o** in the cue list, to edit the **whole demo**.
2. Change `title`, edit the global `[layout]`, add/reorder `[[queues]]`, or give
   an individual cue its own `layout` tree and weights. Every tree must contain
   every pane ID exactly once.
3. Press **Ctrl-S** or **F3**, enter a path such as `my-demo.toml`, then Enter.
   This saves and applies the demo, resetting cue progress while retaining
   matching shell sessions. Saving does not execute cues.
4. To load another file later: full editor → **Ctrl-L/F2** → path → Enter →
   **Ctrl-G/F4** to apply.
5. Press **Ctrl-G, q** to exit. Changes are not automatically saved on exit.

Relaunch your saved file with the compiled command:

```sh
nysos --config my-demo.toml --check
nysos --config my-demo.toml
```

You can also export a fresh starter without opening the app:

```sh
nysos --init demo.toml
```

`--init` refuses to overwrite existing files. Paths in the editor are literal;
use a relative or absolute path rather than `~`. Saving rewrites TOML formatting
and drops comments. For all settings, see the
[configuration reference](../reference/configuration.md). **Ctrl-G, ?** opens
in-app help; `nysos --help` and `man nysos` describe command-line usage.
