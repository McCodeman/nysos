# Manual page

The section 1 manual is [nysos.1](../man/nysos.1). It includes command-line options,
examples, interactive controls, configuration, environment variables, and exit
statuses. View it directly from the repository:

```sh
make man
# Equivalent:
man ./docs/man/nysos.1
```

Install it under your user prefix (default `~/.local`):

```sh
make man-install
man nysos
```

If your system's manual search path does not include `~/.local/share/man`, use:

```sh
man -M "$HOME/.local/share/man" nysos
```

Packagers can override `PREFIX`, `MANDIR`, and `DESTDIR`:

```sh
make man-install PREFIX=/usr/local DESTDIR=/tmp/nysos-package
```

The manpage's options and description are generated from `src/cli.rs` through
clap_mangen; its supplementary sections live in `docs/man/extra.roff`.
`make docs-generate` refreshes both the manpage and
[CLI reference](cli.md). `make docs-check` fails if either is stale.
