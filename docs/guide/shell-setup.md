# PATH and shell completions

Install the binary in a lasting location first, then run the setup switches.
They work without an interactive terminal and exit without starting a demo.

```sh
make install
"${CARGO_HOME:-$HOME/.cargo}/bin/nysos" --add-to-path --install-completions
```

Both switches accept `bash`, `zsh`, or `all`. Omitting the value selects `all`.
You can run either switch separately:

```sh
nysos --add-to-path zsh
nysos --install-completions bash
nysos --add-to-path all --install-completions all
```

Open a new terminal afterward. A child process cannot change the PATH or
completion definitions of your already-running shell. These switches cannot be
combined with `--config`, `--check`, or `--init`.

## Choose the binary directory

`--add-to-path` adds the directory of the running executable; it does not copy or
install the binary. If you run `target/debug/nysos`, that build directory is what
gets added. For symlinked or versioned installations, choose a stable directory:

```sh
nysos --add-to-path --bin-dir "$HOME/.local/bin"
# For a Nix profile that contains nysos:
nysos --add-to-path --bin-dir "$HOME/.nix-profile/bin"
```

The selected directory must contain a file named `nysos`. Explicit directory
symlinks are preserved, so a profile can point to a newer package after an upgrade.
Without `--bin-dir`, a Nix-built executable can resolve to a version-specific
store directory that later gets garbage-collected. `nix develop` already provides
its toolchain PATH; this installer is intended for use outside that shell.

Paths containing spaces, apostrophes, dollar signs, or glob characters are quoted.
Colons cannot be represented in a Unix PATH entry and are rejected, as are
newlines and non-UTF-8 installation paths. Repeated startup loads do not duplicate
the directory in PATH. If it is already present, its existing position is kept.

## Files updated

| Shell | PATH setup | Completion loading |
| --- | --- | --- |
| Bash | `~/.bashrc` and the first existing login file: `.bash_profile`, `.bash_login`, `.profile`; creates `.profile` if none exists | Same files; guarded so completions only load in interactive Bash |
| Zsh | `$ZDOTDIR/.zprofile` and `$ZDOTDIR/.zshrc`, using `$HOME` when ZDOTDIR is unset | `$ZDOTDIR/.zshrc` |

Bash setup does not create a `.bash_profile` that would hide an existing login
file. Zsh initializes `compinit` only if `compdef` is not already available, then
sources the generated completion function directly. This also works when a shell
framework initialized completions earlier. Existing early `return` or `exit`
statements in startup files can prevent appended blocks from running; move the
managed block above such statements if necessary.

Completion scripts are generated from the same CLI definition as `--help`:

- `$XDG_DATA_HOME/nysos/completions/nysos.bash`
- `$XDG_DATA_HOME/nysos/completions/_nysos`

If `XDG_DATA_HOME` is unset or relative, the base is `~/.local/share`. `HOME` must
be an absolute path; a relative `ZDOTDIR` is resolved from the launch directory.
Rerun `--install-completions` after upgrading nysos to refresh available options.

## Repeated installs, backups, and removal

Startup changes live between comments such as `# >>> nysos path >>>` and
`# <<< nysos path <<<`. Reinstalling replaces only that managed block and preserves
surrounding content. Duplicate or incomplete markers cause an error before any
planned changes are written.

Existing files get a sibling `.nysos.bak` backup before their first replacement.
Backups are never overwritten. Dotfile symlinks remain symlinks; their targets are
updated and backed up. A broken symlink produces an error. Each file replacement
is atomic, but the overall multi-file operation is not: if writing a later file
fails, earlier reported updates remain. Fix the error and rerun the installer.

To undo setup, remove the nysos-marked blocks from the listed startup files and
remove the two generated completion scripts. The backup can recover the original
content, but restoring it also discards unrelated edits made since that backup.
Close and reopen your terminal to discard previously loaded definitions.

Only Bash and Zsh are supported by these switches. Native Windows setup is
deferred; run them inside WSL for a Linux shell.
