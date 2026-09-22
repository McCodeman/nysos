# nysos

Multi-pane scripted and interactive CLI demonstrations in Rust, built with
Alacritty's terminal engine, Ratatui, Crossterm, and portable-pty. Every named pane
owns a real shell. Advance prepared commands manually, edit or skip them, and add
ad hoc panes while presenting.

macOS first, Linux second; native Windows interaction is deferred (use WSL).
The Alacritty application itself is optional.

Use `nysos --version` or `--version-full` for build and Git information;
`nysos -V` prints the compact version.

## Quick start

Install [Nix](https://nixos.org/download/), then use the locked toolchain:

```sh
nix develop
make build
make run
```

Press **Ctrl-G**, release, then **n** to run the next command. **Ctrl-G, ?**
shows controls; **Ctrl-G, q** quits.
The left cue list supports **Up/Down**, **Enter** to run a cue, and **e** to edit
it. Press **o** in the list (or **Ctrl-G, o** anywhere) to edit the full demo,
add cues, and change its title; Ctrl-L loads a file and Ctrl-S saves.
**Ctrl-G, 0** focuses the list; **Ctrl-G, c** shows or hides it.
Ctrl-G avoids tmux’s default Ctrl-B. Use `--prefix` to customize it; see
[tmux and SSH](docs/guide/tmux-ssh.md) for remote setup and mouse forwarding.
Ghostty Option-arrows are detected automatically; use `--terminal-keys ghostty`
when tmux/SSH hides the terminal identity, or `standard` to preserve Alt-B/F.
Click or use Alt-Left/Right to focus panes; drag shared borders to resize.

```sh
cargo run -- --init demo.toml
cargo run -- --config demo.toml --check
make run ARGS='--config demo.toml'
```

`--init` refuses to overwrite files. No demo file is loaded automatically. Each
advance sends one command to its target pane's current foreground program; wait
for the shell prompt before advancing.

After installing the binary, enable it in your shells:

```sh
nysos --add-to-path --install-completions
```

Use the binary's full path if it is not yet on PATH. Reopen your terminal afterward.

## Documentation

- [Getting started](docs/guide/getting-started.md)
- [PATH and Bash/Zsh completions](docs/guide/shell-setup.md)
- [Presenting a demo](docs/guide/presenting.md)
- [Keyboard and mouse controls](docs/guide/controls.md)
- [Editing, loading, and saving](docs/guide/editor.md)
- [Complete demo configuration reference](docs/reference/configuration.md)
- [CLI reference](docs/reference/cli.md) — also `nysos --help`
- [Manpage](docs/reference/manpage.md) — `make man` or `make man-install`
- [Troubleshooting](docs/guide/troubleshooting.md)

The Zensical site is configured by `zensical.toml`, with all content in `docs/`.
Inside `nix develop`, Python and uv are already available:

```sh
make docs-setup
make docs-serve    # http://127.0.0.1:8000
make docs-build    # static site in site/
```

## Development

```sh
make              # Grouped target help
make verify       # Rust checks, tests, config validation, reference freshness
make hooks        # Activate the staged-snapshot pre-commit hook
```

Use `nix build` for the packaged release binary and manpage in `result/`, or
`nix flake check` for the sandboxed package and quality checks.

See [development documentation](docs/development.md) for build targets, generated
CLI/manpage updates, CI, and the code map. The annotated
[example demo](examples/demo.toml) uses `/bin/zsh`; change that to an installed
shell on Linux, or use `--init` to generate a demo using `$SHELL`.
