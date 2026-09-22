# Development and documentation

Run `make` or `make help` for grouped, self-documenting targets. Each public target
has a `##` comment; group headings use `##@`. When adding targets, follow that
convention so the help stays current.

## Nix toolchain

Install [Nix](https://nixos.org/download/) with flakes and the `nix-command`
interface enabled. If your installation requires it, add
`experimental-features = nix-command flakes` to `~/.config/nix/nix.conf`.

```sh
nix develop                 # Or: make dev
make verify
make docs-build
```

### Automatic environment with direnv

The repository's `.envrc` uses `use flake .` to load the same locked development
shell automatically. Install direnv separately, then enable its hook in your
interactive shell if it is not already present:

```sh
# Zsh: add to ~/.zshrc once
eval "$(direnv hook zsh)"

# Bash: use this instead in ~/.bashrc
eval "$(direnv hook bash)"
```

Open a new shell, enter the repository, and run `direnv allow` once to trust its
`.envrc`. Future directory changes load the Nix tools automatically and restore
the previous environment when you leave. In a shell already inside the directory,
the next prompt runs the hook. `direnv status` shows the current authorization.
Run `command -v cargo` after entry; it should resolve into `/nix/store/`.

Changes to `flake.nix`, `flake.lock`, `Cargo.toml`, and `Cargo.lock` trigger a
reload. If `.envrc` changes, inspect it and run `direnv allow` again. Generated
profiles live under `.direnv/`, which is ignored by Git. The initial activation
may download dependencies. Standard direnv's flake support is sufficient;
nix-direnv is an optional caching enhancement. `nix develop` remains available
for shells without a direnv hook and for CI.

### Toolchain contents

`flake.lock` pins Nixpkgs and the development/build dependencies. The shell
provides Rust, Cargo, rustfmt, Clippy, rust-analyzer, Make, Git, pkg-config,
Python 3.13, uv, mandoc, Zsh, and nixfmt. Darwin gets an Apple SDK and libiconv;
Linux gets xdg-utils for URL opening. Python downloads are disabled: uv uses the
Nix-provided interpreter, while `uv.lock` pins Zensical and its Python packages.
Cargo dependencies remain pinned by `Cargo.lock`.

Supported outputs: Apple Silicon macOS, Intel macOS, ARM64 Linux, and x86-64
Linux. Intel macOS uses the maintained Nixpkgs 26.05 branch because current
Nixpkgs has removed that platform; other platforms use a pinned unstable
revision. Cross-platform outputs are evaluated, but each binary must be built
on its native platform or through a matching remote builder.

```sh
nix build                   # result/bin/nysos and result/share/man/man1/nysos.1.gz
nix run -- --help
nix flake check              # Package, real-PTY tests, formatting, lint, references
nix fmt                     # Format flake.nix
nix develop --command make verify
```

Nix package builds vendor the locked Cargo dependencies and run without network
access after fetching inputs. They exclude local build artifacts and virtual
environments. PTY tests use `NYSOS_TEST_SHELL` supplied by Nix rather than assuming
`/bin/sh` exists in the sandbox. This variable only affects tests. `NYSOS_TEST_BASH` and `NYSOS_TEST_ZSH`
select the shells used to test installer snippets and completion loading.

Documentation builds run through `nix develop` and may fetch the locked Python
packages using uv; the docs site is not a sandboxed Nix derivation. CI uses the
same flake for macOS/Linux checks and the documentation build.

Flakes in Git repositories see only files known to Git. Stage new files (or use
`git add -N` for intent-to-add) before evaluating changes. The pre-commit hook
enters the pinned shell from a temporary export of the staged index, so it also
works when Git is invoked outside a development shell. Install Nix before enabling
the hook with `make hooks`.

Update Nix dependencies intentionally, then verify and commit the lockfile:

```sh
nix flake update
nix flake check
nix develop --command make verify
nix develop --command make docs-build
```

If an existing `.venv` refers to a removed or incompatible interpreter, remove
that generated virtual environment and rerun `make docs-setup` inside the shell.
Keep `flake.lock`, `Cargo.lock`, and `uv.lock` in version control. `make clean`
cleans Cargo artifacts, not the Nix store; Nix garbage collection is managed
separately by your Nix installation.

## Rust workflow

```sh
make build
make run ARGS='--config examples/demo.toml'
make verify
make hooks
```

`make verify` checks formatting, Clippy, unit/PTY tests, the example configuration,
and generated CLI/manpage freshness. The pre-commit hook exports the Git index to
a temporary directory and runs this inside the locked Nix shell against exactly the staged snapshot, including
partially staged files. It reuses `target/pre-commit`. Stage regenerated reference
files alongside CLI changes. PTY tests need PTY access.

`make build` writes `target/debug/nysos`; `make release` writes
`target/release/nysos`. `make install` installs through Cargo and respects
`CARGO_HOME`. `PREFIX` and `DESTDIR` affect the manpage installation only.

## Documentation workflow

Enter `nix develop` to use the pinned Python and uv tools. The documentation environment is separate from Rust dependencies;
`uv.lock` locks Zensical and its Python dependencies.

```sh
make docs-setup
make docs-serve
```

Open `http://127.0.0.1:8000`. Override the preview address if needed:

```sh
make docs-serve DOCS_ADDR=127.0.0.1:8080
make docs-build
```

The [Zensical](https://zensical.org/docs/) configuration is `zensical.toml` at the
repository root. Content lives in `docs/`, and the built site goes to ignored
`site/`. Add new pages to the explicit `nav` in `zensical.toml`. The strict build
checks the site; CI builds it without publishing. Set `project.site_url` once a
hosting location is chosen. No deployment target is configured.

The theme uses system fonts, so it does not need to fetch web fonts during builds.
`make docs-clean` removes the generated site and Zensical cache, while retaining
the virtual environment, source pages, and checked-in reference artifacts.

## Generated CLI reference and manpage

`src/cli.rs` owns option names, descriptions, examples, and long help. The executable
and `examples/generate_docs.rs` both use that definition. The generator also reads
`docs/man/extra.roff` for the manual's interactive controls and other supplementary
sections.

```sh
make docs-generate
make docs-check
make man
```

Check in both generated files, `docs/reference/cli.md` and `docs/man/nysos.1`, when
changing their inputs. `make docs-check` compares their contents without modifying
them, and is included in both Rust verification and the site build. The optional
command `mandoc -T lint docs/man/nysos.1` checks roff syntax when mandoc is installed.

The [manpage page](reference/manpage.md) explains local viewing and installation.

## Code map

| Module | Responsibility |
| --- | --- |
| `src/cli.rs` | CLI definition and help text |
| `src/main.rs` | Startup, terminal restoration, and event loop |
| `src/config.rs` | TOML schema, validation, and atomic saves |
| `src/pane.rs` | PTY lifecycle, terminal parser, rendering, URLs |
| `src/input.rs` | Keyboard and application mouse encoding |
| `src/layout.rs` | Weighted layouts and divider resizing |
| `src/app.rs` | Queue position, focus, modal editor, and UI |
| `src/shell_setup.rs` | User PATH/completion installation and managed startup blocks |
| `src/platform.rs` | Local macOS Command-modifier detection |

## Build identity

`nysos --version` (also `--version-full`) reports the package version, full Git
commit, tags pointing at that commit, nearest-tag description, Rust compiler,
target triple, and build profile/optimization level. `nysos -V` prints just the
name and package version. These commands work without a terminal or Git installed
at runtime; metadata is embedded by `build.rs`.

Cargo builds discover Git metadata from the project checkout, including
lightweight tags. Missing Git, uncommitted repositories, and source archives
report unavailable values as `unknown`. Tag output lists exact tags; the Git
description can instead show the nearest reachable tag and commit distance.
Git metadata identifies the base commit, not a guarantee of a clean worktree.

Nix builds receive the flake revision (including a dirty suffix when supplied by
Nix). Since flake sources omit `.git`, tags are `unknown`. Other archive/release
builders can set `NYSOS_GIT_COMMIT`, `NYSOS_GIT_TAG`, and `NYSOS_GIT_DESCRIBE` at
build time. Setting `NYSOS_GIT_COMMIT` disables Git discovery; omitted metadata
then remains `unknown`. No current timestamp is embedded, preserving reproducible
builds. Regenerate CLI docs with `make docs-generate`; generated references omit
machine-specific build metadata.

## Releases

[Release Please](https://github.com/googleapis/release-please) runs on pushes to
`main` and can be started manually from Actions. Its Rust strategy maintains a
release PR with `Cargo.toml`, the root package in `Cargo.lock`, `CHANGELOG.md`,
and `.release-please-manifest.json`. The generated manpage has invisible version
markers so its version stays synchronized without hand-editing generated output.

Use Conventional Commits for commits merged into `main` (or squash PR titles):

- `fix: ...` produces a patch release.
- `feat: ...` produces a minor release.
- `feat!: ...` or a `BREAKING CHANGE:` footer produces a minor release while
  below 1.0.0, then a major release afterward.
- `docs: ...`, `ci: ...`, and `chore: ...` alone do not trigger a release.

The initial manifest records the existing 0.1.0 package. The bootstrap commit is
`c74a829`; older commits used non-conventional messages and are excluded from
initial automated release notes. This setup does not create a historical v0.1.0
tag or publish a release immediately. The next qualifying commit opens a release
PR; subsequent commits update it. Review its changelog and version, wait for CI,
and merge it to publish the `vX.Y.Z` Git tag and GitHub release.

The workflow uses the repository's built-in `GITHUB_TOKEN`, with write access
scoped to this workflow. Repository Actions settings must allow Actions to create
pull requests. Token-created PRs do not trigger ordinary PR workflows, so the
release workflow explicitly dispatches **Build and docs** on the release branch.
Review that run in Actions before merging; dispatch runs may not appear as normal
PR-required checks. No personal access token is required. This setup does not
publish to crates.io or attach prebuilt binaries.

After a release, update the separate
[Homebrew tap](https://github.com/McCodeman/homebrew-tap) to the new tag archive,
checksum, and Git build metadata, following its maintenance instructions.
Homebrew formula updates are currently manual.
