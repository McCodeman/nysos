<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Contributing to nysos

Bug reports, documentation improvements, and focused pull requests are welcome.
Discuss large behavior or architecture changes in an issue before implementing
them. For vulnerabilities, follow [SECURITY.md](SECURITY.md).

## Development

1. Install Nix with flakes enabled and clone the repository.
2. Run `nix develop`, or enable direnv and run `direnv allow`.
3. Run `make hooks` to enable checks against exactly the staged files.
4. Make a focused change and add tests for new behavior or regressions.
5. Run `make verify` and `make docs-build`. Run `make test-focus test-tmux` when
   changing input, focus, panes, or terminal behavior.

Use `make` to list grouped build targets. macOS is the primary platform, Linux
is supported, and native Windows support is deferred. Include your OS, terminal,
tmux/SSH usage, and `nysos --version-full` when reporting a terminal issue.
Remove secrets and sensitive command output from reproductions.

## Pull requests and commits

Explain the problem, resulting behavior, and validation performed. Use
Conventional Commits such as `feat: add ...`, `fix: handle ...`, and `docs: explain ...`;
Release Please uses them to prepare releases. Keep unrelated changes separate.

Sign commits with Sigstore/gitsign using the setup in
[the development guide](docs/development.md#sigstore-git-signatures). Signing
requires an internet connection and identity authentication. Do not rewrite
published shared history to add signatures to old commits.

## Licensing and generated files

Contributions are accepted under Apache-2.0, the project's license. Submit only
work you have the right to contribute, and retain existing attribution notices.
No copyright assignment is required. Include the project's copyright and SPDX
headers in source files; additional contributors may add their own attribution.
Run `make license-check` to check the required headers.

Edit `src/cli.rs`, `docs/man/extra.roff`, or the generator rather than directly
editing generated CLI/manpage files; run `make docs-generate` afterward.
Run `make sbom` when dependencies or package licensing change. The SPDX SBOM
records dependency licenses; it does not relicense third-party code.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md).
