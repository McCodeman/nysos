<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# nysos

[![CI](https://github.com/McCodeman/nysos/actions/workflows/ci.yml/badge.svg)](https://github.com/McCodeman/nysos/actions/workflows/ci.yml)
[![Release Please](https://github.com/McCodeman/nysos/actions/workflows/release-please.yml/badge.svg)](https://github.com/McCodeman/nysos/actions/workflows/release-please.yml)
[![Latest tag](https://img.shields.io/github/v/tag/McCodeman/nysos)](https://github.com/McCodeman/nysos/tags)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![SPDX SBOM](https://img.shields.io/badge/SBOM-SPDX%202.3-blue.svg)](sbom/nysos.spdx.json)
[![Homebrew](https://img.shields.io/badge/Homebrew-mccodeman%2Ftap-orange.svg)](https://github.com/McCodeman/homebrew-tap)

Multi-pane scripted and interactive CLI demonstrations in Rust, built with
Alacritty's terminal engine, Ratatui, Crossterm, and portable-pty. Every named pane
owns a real shell. Advance prepared commands manually, edit or skip them, and add
ad hoc panes while presenting.

macOS first, Linux second; native Windows interaction is deferred (use WSL).
The Alacritty application itself is optional.

Use `nysos --version` or `--version-full` for build and Git information;
`nysos -V` prints the compact version.

## Quick start

Plain `nysos` opens two interactive shells with an empty cue list. Use
`nysos --demo` to load the built-in six-cue example of changing pane titles and
themes. `nysos --init demo.toml` exports that example for editing.

Install with Homebrew on macOS or Linux:

```sh
brew install mccodeman/tap/nysos
nysos --init demo.toml
nysos --config demo.toml
```

The [Homebrew tap](https://github.com/McCodeman/homebrew-tap) builds from source
using Homebrew's Rust dependency; Nix is not required. It also installs
`man nysos`. To update, run `brew update && brew upgrade nysos`.
For optional Bash and Zsh completions, run `nysos --install-completions` and
reopen your terminal.

### Build from source

Install [Nix](https://nixos.org/download/), then use the locked toolchain:

```sh
nix develop
make build
make run
```

With direnv installed and its shell hook enabled, run `direnv allow` once in
this directory. The checked-in `.envrc` then loads the Nix development shell on
entry and restores your previous environment on exit. See
[development setup](docs/development.md#automatic-environment-with-direnv).

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

Pane commands target a stable `id`; `title` controls the visible label. Each cue's
commands can override `title` and `scheme` without restarting the pane:

```toml
[[panes]]
id = "server"
title = "Server shell"
scheme = "ocean"

[[queues]]
name = "Inspect logs"
commands = [
  { pane = "server", command = "tail -20 app.log", title = "Server logs", scheme = "forest" },
]
```

## Documentation

Try the [pane transitions demo](examples/pane-transitions.toml) for six cues that
change titles and themes independently in two live shells:

```sh
nysos --config examples/pane-transitions.toml
```

It includes updates to one pane, both panes, just a title, and just a theme.
Press Enter in the cue list to run each cue; omitted panes keep their state.

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
shell on Linux. The built-in `--demo` and exported `--init` sample use `/bin/sh`.

Releases use [Release Please](docs/development.md#releases). Use Conventional
Commits (`feat:`, `fix:`) so it can prepare version bumps and release notes;
merging its release PR publishes the GitHub release.

## Community and licensing

Read [CONTRIBUTING.md](CONTRIBUTING.md) to contribute,
[SUPPORT.md](SUPPORT.md) for help, [SECURITY.md](SECURITY.md) to report a
vulnerability privately, and our [Code of Conduct](CODE_OF_CONDUCT.md).

Copyright 2026 Marshall Cody McCain (mccodeman@proton.me).
Licensed under [Apache-2.0](LICENSE); see [NOTICE](NOTICE). Third-party dependencies
retain their own licenses. Earlier tagged versions retain their published licenses.
Generate the Rust dependency [SPDX SBOM](sbom/nysos.spdx.json) with `make sbom`;
CI also publishes a freshly generated SBOM artifact.
