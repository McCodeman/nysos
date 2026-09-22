<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

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
make test-layout
make hooks
```

`make verify` checks source headers, workflow/installer lint, formatting, Clippy,
unit/PTY tests, the example configurations,
and generated CLI/manpage freshness. The pre-commit hook exports the Git index to
a temporary directory and runs this inside the locked Nix shell against exactly the staged snapshot, including
partially staged files. It reuses `target/pre-commit`. Stage regenerated reference
files alongside CLI changes. PTY tests need PTY access.

`make build` writes `target/debug/nysos`; `make release` writes
`target/release/nysos`. `make install` installs through Cargo and respects
`CARGO_HOME` (normally `~/.cargo`). It replaces the installed binary even when
its version number has not changed. `PREFIX` and `DESTDIR` affect the manpage
installation only.

To test local changes while keeping Homebrew installed:

```sh
make install
"${CARGO_HOME:-$HOME/.cargo}/bin/nysos" --add-to-path=zsh
exec zsh -l
command -v nysos
nysos --version-full
```

The PATH setup is needed once; it moves the Cargo binary directory ahead of
Homebrew in new shells, even if that directory was already on PATH. Use
`--add-to-path=bash` for Bash. Subsequent `make install` runs update the binary
used by `nysos` without changing the Homebrew package. If direnv has not loaded
the toolchain, run `nix develop --command make install`.

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
| `src/features.rs` | Typed runtime feature-gate registry and startup override precedence |
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
then remains `unknown`.

Full version output includes `Build time (UTC): YYYY-MM-DDTHH:MM:SSZ`. Ordinary
Cargo builds embed the time the build script runs; unchanged cached binaries
retain their original timestamp. Source, manifest, lockfile, and Git metadata
changes rerun the build script. Reproducible builders can set `SOURCE_DATE_EPOCH`
to nonnegative Unix seconds; that timestamp is explicitly marked in the output.
Nix supplies this variable, so Nix builds show the reproducible source timestamp
rather than the wall-clock compilation time. Invalid values fail the build.

Regenerate CLI docs with `make docs-generate`; generated references omit
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
initial automated release notes. The initial `v0.1.0` tag was subsequently signed with Sigstore and published.
Release Please does not publish a new release immediately. The next qualifying commit opens a release
PR; subsequent commits update it. Review its changelog and version, wait for CI,
and merge it to publish the `vX.Y.Z` Git tag and GitHub release.

The workflow uses the repository's built-in `GITHUB_TOKEN`, with write access
scoped to this workflow. Repository Actions settings must allow Actions to create
pull requests. Token-created PRs do not trigger ordinary PR workflows, so the
release workflow explicitly dispatches **Build and docs** on the release branch.
Review that run in Actions before merging; dispatch runs may not appear as normal
PR-required checks. No personal access token is required. This setup does not
publish to crates.io. It now builds precompiled archives and Debian/RPM packages
before publishing the release draft.

After a release, update the separate
[Homebrew tap](https://github.com/McCodeman/homebrew-tap) to the new tag archive,
checksum, and Git build metadata, following its maintenance instructions.
Homebrew formula updates are currently manual.

### Sigstore Git signatures

This checkout signs commits and tags with `gitsign`. New clones can enable the
same configuration inside `nix develop` (which includes gitsign):

```sh
git config --local gpg.x509.program gitsign
git config --local gpg.format x509
git config --local commit.gpgsign true
git config --local tag.gpgsign true
```

Signing requires internet access and browser authentication. Signatures record
the authenticated identity in Sigstore's public transparency log. Existing
commits are not rewritten. Verify the first signed tag with:

```sh
gitsign verify-tag v0.1.0 \
  --certificate-identity mccodeman@proton.me \
  --certificate-oidc-issuer https://github.com/login/oauth
```

Release Please initially writes its PR through GitHub's API; the workflow then
amends the generated release-branch commit with a Sigstore signature using GitHub
Actions OIDC before dispatching CI. A lease prevents overwriting concurrent
branch updates. The bot signer identity is the release workflow, not a personal
email. Check that this signing step passes before accepting a release PR.

GitHub UI squash/merge operations create new commits with GitHub's signing
system, not Sigstore. To retain a Sigstore-signed release commit, fast-forward
`main` locally to the reviewed release branch and push it; if main has advanced,
let Release Please refresh its branch first. Locally created merge commits must
also be signed. Release Please's automatically created release tags use its API
and are not Sigstore-signed; the manually created `v0.1.0` tag is signed.

## Licensing and dependency inventory

First-party code and documentation use Apache-2.0. Preserve the copyright and
SPDX headers; `make license-check` is included in `make verify` and pre-commit.
JSON and generated lockfiles use adjacent `.license` sidecars where inline
comments are unavailable or would be overwritten. The canonical `LICENSE` text
is unmodified; `NOTICE` records project attribution. Nix packages include both.
See the root contributing, security, support, and conduct documents for project
policies.

`make sbom` generates `sbom/nysos.spdx.json` with the locked `cargo-sbom` tool,
then validates it with `spdx-tools`. `make sbom-check` verifies SPDX validity,
normal/build dependency coverage, the lockfile digest, and package metadata.
See `sbom/README.md` in the source checkout for scope and exclusions. SBOM checks
run separately in CI because they require Python tooling and Cargo dependency
metadata. Release PRs regenerate the snapshot before the bot signs its commit,
and release drafts receive a freshly generated SPDX asset before publication.
This ordering is required by GitHub immutable releases. v0.1.1 was published
before its asset upload; its release notes link to the SBOM in the signed tag
instead. That SBOM is also present in its source archives.

Licensing changes apply to this source revision and future releases. Previously
published tags and the Homebrew formula pinned to an older MIT-licensed commit
retain their original licensing; update the tap's license together with its
source revision when packaging a new Apache-2.0 release.

`make test-layout` uses a private tmux server and a real client PTY to check nested
mouse dragging, keyboard-only operation with `--no-mouse`, child `stty size`
reports after terminal size changes, and saving/applying resized layout trees.
It models the cell-size notifications produced by font zoom, without controlling
a graphical terminal's font settings. SSH environment variables exercise the
remote code path; this is not an end-to-end SSH network test.

## Release archives and Linux packages

The Nix development shell includes nFPM. Linux builds additionally need Docker
with Buildx and a running daemon (Docker Desktop or OrbStack on macOS). The pinned
Rust/Alpine image in `packaging/Dockerfile` builds static musl executables with an
explicit target so build-time procedural macros remain dynamically loadable.
The Linux archive runs on both musl and glibc distributions.

```sh
nix develop
make package-linux
make package-check PACKAGE_PLATFORM=linux-amd64
make package-check PACKAGE_PLATFORM=linux-arm64
make test-install
```

Outputs are in ignored `dist/`: two architectures, each with `.tar.gz`, `.deb`,
and `.rpm`, plus `SHA256SUMS`. `make package PACKAGE_PLATFORM=darwin-arm64` builds
a macOS archive on Apple Silicon; use `darwin-amd64` on Intel. All archives include
the executable, manpage, Bash/Zsh completions, examples, LICENSE/NOTICE, and SPDX
SBOM. Run `make sbom` if the locked dependency graph or package version changes.
The CLI/doc generator remains the source for packaged completions and help.

The reusable **Release packages** workflow builds on matching Linux/macOS runners.
The release workflow creates a draft, waits for all package jobs, gathers their
artifacts, adds `install.sh`, SPDX and `SHA256SUMS`, then publishes. This ordering
supports immutable GitHub releases. A failed package job leaves the release a
draft; inspect and rerun that failed workflow before publishing anything manually.
The reusable workflow can also be dispatched for a commit/tag to build artifacts
without publishing a release. The local packaging commands do not create tags,
commits, GitHub releases, apt repositories, or yum repositories.

`make package-check` installs Linux packages in disposable Debian, Ubuntu, and
Fedora containers, then starts the portable binary's real PTY demo under Alpine.
`make test-install` mocks HTTPS downloads locally and tests checksum rejection,
missing assets, unsafe archive paths, repeated installation, and quoted prefixes.
It does not download or install onto the host. The real installer modifies only
the requested prefix and does not silently edit PATH or invoke sudo.

The replay regression acknowledges each dispatched cue through its shell before
sending another. Flooding macOS PTYs with dozens of unconsumed command lines can
lose input under CI load, which previously made that test flaky. This does not
add command-completion detection to the app. CI uses the standard Nix binary
cache and does not require a FlakeHub account or its optional cache action.

Intel macOS uses the locked Nixpkgs 26.05 toolchain and installer v3.12.2.
The flake input named `nixpkgs` also supplies the Bash that `nix develop` uses
to enter the environment; keeping it on 26.05 avoids falling back to macOS's
old system Bash. Other platforms use the separately locked `nixpkgs-current`
packages. The resize integration test waits for nysos's rendered pane corner
after tmux resizes, so shell-size assertions wait for the application's redraw.

## Experimental feature gates

`src/features.rs` defines the closed `Feature` enum shared by CLI and TOML.
Experimental behavior must be inert when its gate is disabled.
`features::defaults()` defines the enabled defaults; currently `line-numbers` is
on by default. Explicit TOML arrays replace that list, including `[]` to opt out. Add the variant, documentation, and tests
for disabled behavior, enabled behavior, and CLI disable precedence. Gates are
runtime settings; Cargo artifacts and packages contain the same capabilities.
The full editor can change the list, and saves persist effective startup overrides.
The line-number gate's geometry is centralized in `Pane::body_area` so rendering,
PTY sizing, cursor placement, and mouse translation agree. `make test-tmux`
checks live toggling and the child PTY width with the gate enabled.
