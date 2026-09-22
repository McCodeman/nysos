<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Installation and packages

The [Getting Started guide](getting-started.md) has Homebrew and clone/install
instructions followed by a hands-on demo. This page covers prebuilt releases.

**Availability:** the historical v0.1.1 release has no prebuilt assets. The new
release pipeline builds the packages described below for subsequent releases.
Until such a release is published, install from the source checkout. The installer
reports missing assets clearly and leaves the existing executable untouched.

## Download-and-install script

After a release with binary assets is published, paste this into your shell:

```sh
NYSOS_INSTALLER=$(mktemp)
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/McCodeman/nysos/releases/latest/download/install.sh \
  -o "$NYSOS_INSTALLER" && sh "$NYSOS_INSTALLER"
export PATH="$HOME/.local/bin:$PATH"
nysos --add-to-path --bin-dir "$HOME/.local/bin" --install-completions
nysos --demo
```

The script downloads the selected archive and `SHA256SUMS` from GitHub, verifies
its SHA-256 digest, then installs the binary, manual, completions, examples,
license, and SPDX SBOM under `~/.local`. It does not invoke sudo or edit startup
files; the separate `--add-to-path` command handles persistent shell setup.
A checksum detects download corruption; it is not an independent signature.
Read the downloaded script before running it if you want to inspect its actions.

Choose a published release and another writable prefix:

```sh
sh "$NYSOS_INSTALLER" --version vX.Y.Z --prefix "$HOME/apps/nysos"
export PATH="$HOME/apps/nysos/bin:$PATH"
```

Replace `vX.Y.Z` with a tag listed on [GitHub Releases](https://github.com/McCodeman/nysos/releases).
The installer supports Linux and macOS, x86-64 and ARM64. It requires a POSIX shell,
`curl`, `tar`, and either `sha256sum` or `shasum`, plus standard Unix file utilities.
It rejects unsupported platforms, missing/mismatched checksums, and unsafe archive
paths before installation. Rerun to upgrade; the executable is replaced atomically
but documentation/completion installation is not a single transaction. Keep a
previous release archive if you want to reinstall an older version.

## Native Linux packages

Release assets use `amd64` for x86-64 and `arm64` for ARM64. Linux binaries are
statically linked with musl, so the archives do not depend on a particular glibc
version. A usable shell such as `/bin/sh` and an interactive terminal are still
required. Linux packages install the binary at `/usr/bin/nysos` and supporting
files under `/usr/share`.

Download the matching package and `SHA256SUMS` from the same release. Set the
version below to that release's number, without its leading `v`:

=== "Debian / Ubuntu (apt)"

    ```sh
    NYSOS_VERSION=X.Y.Z
    NYSOS_ARCH=$(dpkg --print-architecture)
    NYSOS_PACKAGE="nysos_${NYSOS_VERSION}_linux-${NYSOS_ARCH}.deb"
    curl -fLO "https://github.com/McCodeman/nysos/releases/download/v${NYSOS_VERSION}/${NYSOS_PACKAGE}"
    curl -fLO "https://github.com/McCodeman/nysos/releases/download/v${NYSOS_VERSION}/SHA256SUMS"
    awk -v file="$NYSOS_PACKAGE" '$2 == file' SHA256SUMS | sha256sum -c - &&
      sudo apt install "./$NYSOS_PACKAGE"
    nysos --demo
    ```

    Upgrade by installing the newer `.deb`. Remove with `sudo apt remove nysos`.
    Only `amd64` and `arm64` are built.

=== "Fedora / RHEL family (dnf)"

    ```sh
    NYSOS_VERSION=X.Y.Z
    case "$(uname -m)" in
      x86_64) NYSOS_ARCH=amd64 ;;
      aarch64) NYSOS_ARCH=arm64 ;;
      *) echo 'Unsupported architecture'; return 1 ;;
    esac
    NYSOS_PACKAGE="nysos_${NYSOS_VERSION}_linux-${NYSOS_ARCH}.rpm"
    curl -fLO "https://github.com/McCodeman/nysos/releases/download/v${NYSOS_VERSION}/${NYSOS_PACKAGE}"
    curl -fLO "https://github.com/McCodeman/nysos/releases/download/v${NYSOS_VERSION}/SHA256SUMS"
    awk -v file="$NYSOS_PACKAGE" '$2 == file' SHA256SUMS | sha256sum -c - &&
      sudo dnf install "./$NYSOS_PACKAGE"
    nysos --demo
    ```

    `yum install ./package.rpm` or `zypper install ./package.rpm` can install the
    local RPM on systems using those managers. Remove with `sudo dnf remove nysos`.

=== "Other Linux distributions"

    Use the download script above or extract the matching
    `nysos_X.Y.Z_linux-amd64.tar.gz` / `nysos_X.Y.Z_linux-arm64.tar.gz` under a
    prefix such as `~/.local`. Archives include `bin/` and `share/` directories.
    Verify the archive against the release's `SHA256SUMS` before extracting it.
    This also supports Alpine Linux; no glibc compatibility layer is needed.

These are downloadable packages, not an apt/yum repository: `apt install nysos`
without the `./downloaded-file.deb` path is not configured by this project.
The current pipeline does not sign RPM/Debian packages or publish repository
metadata. Use GitHub's HTTPS release downloads and their checksums. Distribution
policies may require additional local package approval.

## macOS archives

Homebrew remains the primary macOS installation route. Release archives are also
built for Apple Silicon (`darwin-arm64`) and Intel (`darwin-amd64`), and the shell
installer selects one automatically. They are command-line archives, not a
notarized application bundle. Source installation remains available if local
platform policy blocks a downloaded executable.

## PATH, completions, and upgrades

```sh
command -v nysos
nysos --version-full
nysos --install-completions
```

If a Cargo or `~/.local` installation is ahead of `/usr/bin` or Homebrew, upgrading
the system package will not change which binary runs. Use `command -v nysos` to
check. Persistent PATH setup moves the selected directory to the front without
duplicating it. See [PATH and completions](shell-setup.md).

Keep installation methods separate: use `make install` for a source build, the
script for a user-prefix archive, or the relevant package manager for its package.
The archive installer does not maintain a package-manager database. To remove an
archive installation, remove its `bin/nysos`, `share/doc/nysos`,
`share/nysos`, `share/man/man1/nysos.1`, and the two nysos completion files.
Do not remove a shared `bin` or `share` directory. Remove any nysos-managed startup
blocks separately if you no longer want them.
