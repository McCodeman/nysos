#!/bin/sh
# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

# Install a checksum-verified GitHub release; no sudo or shell startup edits.
set -eu
main() {
    version=latest
    prefix=${HOME:?HOME must be set}/.local
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version|--prefix)
                [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; return 1; }
                case "$1" in --version) version=$2 ;; --prefix) prefix=$2 ;; esac
                shift 2 ;;
            -h|--help)
                echo 'Usage: sh install.sh [--version vX.Y.Z] [--prefix /absolute/path]'
                echo 'Default: latest published release, installed under ~/.local. No sudo.'
                return 0 ;;
            *) echo "Unknown option: $1" >&2; return 1 ;;
        esac
    done
    case "$prefix" in /*) ;; *) echo '--prefix must be absolute' >&2; return 1 ;; esac
    case "$(uname -s)" in Linux) os=linux ;; Darwin) os=darwin ;; *) echo 'Supported systems: Linux and macOS' >&2; return 1 ;; esac
    case "$(uname -m)" in x86_64|amd64) arch=amd64 ;; arm64|aarch64) arch=arm64 ;; *) echo 'Supported CPUs: x86-64 and ARM64' >&2; return 1 ;; esac
    for tool in curl tar awk mktemp; do
        command -v "$tool" >/dev/null || { echo "Required tool not found: $tool" >&2; return 1; }
    done
    if command -v sha256sum >/dev/null; then
        checksum=sha256sum
    elif command -v shasum >/dev/null; then
        checksum=shasum
    else
        echo 'Install sha256sum or shasum before continuing' >&2
        return 1
    fi
    releases=https://github.com/McCodeman/nysos/releases
    if [ "$version" = latest ]; then
        latest=$(curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o /dev/null -w '%{url_effective}' "$releases/latest")
        version=${latest##*/}
    fi
    case "$version" in v*) ;; *) version=v$version ;; esac
    case "$version" in *[!A-Za-z0-9.+-]*|v) echo 'Invalid version tag' >&2; return 1 ;; esac
    archive=nysos_${version#v}_${os}-${arch}.tar.gz
    work=$(mktemp -d "${TMPDIR:-/tmp}/nysos-install.XXXXXXXX")
    trap 'rm -rf "$work"' EXIT
    trap 'exit 1' HUP INT TERM
    echo "Downloading $version for $os/$arch"
    for asset in "$archive" SHA256SUMS; do
        if ! curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "$releases/download/$version/$asset" -o "$work/$asset"; then
            echo "Release $version does not provide $asset. Choose a release with binaries or install from source." >&2
            return 1
        fi
    done
    expected=$(awk -v file="$archive" '$2 == file && NF == 2 { print $1 }' "$work/SHA256SUMS")
    case "$expected" in ''|*[!0-9a-fA-F]*) echo 'Missing or invalid checksum' >&2; return 1 ;; esac
    [ "${#expected}" -eq 64 ] || { echo 'Invalid checksum length' >&2; return 1; }
    if [ "$checksum" = sha256sum ]; then
        actual=$(sha256sum "$work/$archive" | awk '{print $1}')
    else
        actual=$(shasum -a 256 "$work/$archive" | awk '{print $1}')
    fi
    [ "$actual" = "$expected" ] || { echo 'Checksum mismatch; nothing installed' >&2; return 1; }
    # Release archives contain only regular files and directories, never links.
    tar -tzf "$work/$archive" > "$work/files"
    while IFS= read -r entry; do
        case "$entry" in /*|../*|*/../*|*/..) echo 'Unsafe archive path' >&2; return 1 ;; esac
        case "$entry" in bin|bin/|bin/nysos|share|share/|share/*) ;; *) echo "Unexpected archive path: $entry" >&2; return 1 ;; esac
    done < "$work/files"
    tar -tvzf "$work/$archive" > "$work/types"
    if awk 'substr($0,1,1) != "-" && substr($0,1,1) != "d" { bad=1 } END { exit !bad }' "$work/types"; then
        echo 'Archive contains unsupported entry types' >&2; return 1
    fi
    mkdir "$work/extracted"
    tar -xzf "$work/$archive" -C "$work/extracted"
    [ -f "$work/extracted/bin/nysos" ] && [ -d "$work/extracted/share" ] || { echo 'Incomplete release archive' >&2; return 1; }
    # Metadata goes first; atomically replace the executable last.
    mkdir -p "$prefix/bin" "$prefix/share"
    cp -R "$work/extracted/share/." "$prefix/share/"
    temporary=$(mktemp "$prefix/bin/.nysos.XXXXXXXX")
    if ! cp "$work/extracted/bin/nysos" "$temporary" || ! chmod 755 "$temporary" || ! mv -f "$temporary" "$prefix/bin/nysos"; then
        rm -f "$temporary"
        echo 'Could not install executable' >&2
        return 1
    fi
    echo "Installed $prefix/bin/nysos"
    echo 'Add it to your current shell PATH:'
    # Quote user-chosen paths so the printed command is safe to paste.
    quoted=$(printf '%s' "$prefix/bin" | sed "s/'/'\\\\''/g")
    printf "  export PATH='%s':\"\$PATH\"\n" "$quoted"
    echo 'For persistent PATH and completions, run the installed executable with:'
    printf "  '%s/nysos' --add-to-path --bin-dir '%s' --install-completions\n" "$quoted" "$quoted"
}
main "$@"
