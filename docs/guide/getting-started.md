# Getting started

Install [Nix](https://nixos.org/download/) and enter the project's locked
development shell. It provides Rust, Cargo, formatting/lint tools, Make, Python,
and uv.

```sh
nix develop
make build
make run                           # Two shells using $SHELL, or /bin/sh
make run ARGS='--config examples/demo.toml'
```

The example uses `/bin/zsh` for its presenter pane. On Linux, change that to an
installed shell or generate a starter using your `$SHELL`:

```sh
cargo run -- --init demo.toml       # Refuses to overwrite an existing file
cargo run -- --config demo.toml --check
cargo run -- --config demo.toml
```

Once open, press **Ctrl-G**, release, then **n** to send the next prepared
command. Press **Ctrl-G, ?** for help. **Ctrl-G, q** exits.


For a system-wide command on your user PATH, run `make install`. For an optimized
binary without installation, run `make release` and use `target/release/nysos`.
The starter command writes a file only; launch it explicitly with `--config`.

Continue with [presenting a demo](presenting.md) or the
[configuration reference](../reference/configuration.md).

Use `nix build` to build the packaged release binary and manpage, or `nix run`
to launch it. See [Nix development](../development.md#nix-toolchain) for platforms,
lockfiles, and dependency updates.

To make an installed binary available in new shells and enable tab completion,
see [PATH and shell completions](shell-setup.md).
