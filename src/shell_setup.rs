//! User-scoped, repeatable shell setup. Planning reads all files before mutation.
use crate::cli::{Args, ShellTarget};
use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

struct Locations {
    home: PathBuf,
    zsh: PathBuf,
    data: PathBuf,
}
impl Locations {
    fn from_env() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME must be set for user shell installation")?;
        if !home.is_absolute() {
            bail!("HOME must be an absolute path");
        }
        let zsh = match std::env::var_os("ZDOTDIR").filter(|s| !s.is_empty()) {
            Some(dir) => std::env::current_dir()?.join(dir),
            None => home.clone(),
        };
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        Ok(Self { home, zsh, data })
    }
    fn bash_files(&self) -> Vec<PathBuf> {
        // Bash reads only the first existing login file. Do not create a
        // .bash_profile that would hide an existing .bash_login or .profile.
        let login = [".bash_profile", ".bash_login", ".profile"]
            .into_iter()
            .map(|name| self.home.join(name))
            .find(|path| path.exists() || path.is_symlink())
            .unwrap_or_else(|| self.home.join(".profile"));
        vec![self.home.join(".bashrc"), login]
    }
}

#[derive(Default)]
struct Plan(BTreeMap<PathBuf, String>);
impl Plan {
    fn block(&mut self, path: &Path, name: &str, body: &str) -> Result<()> {
        let path = destination(path)?;
        let previous = match self.0.get(&path) {
            Some(contents) => contents.clone(),
            None => read_optional(&path)?,
        };
        self.0.insert(
            path.clone(),
            managed_block(&previous, name, body)
                .with_context(|| format!("Updating {}", path.display()))?,
        );
        Ok(())
    }
    fn script(&mut self, path: &Path, shell: Shell) -> Result<()> {
        let mut bytes = Vec::new();
        generate(shell, &mut Args::command(), "nysos", &mut bytes);
        self.0.insert(destination(path)?, String::from_utf8(bytes)?);
        Ok(())
    }
    fn apply(self) -> Result<()> {
        for (path, contents) in self.0 {
            if path.exists() && std::fs::read(&path)? == contents.as_bytes() {
                println!("Unchanged: {}", path.display());
                continue;
            }
            write_with_backup(&path, &contents).with_context(|| {
                format!(
                    "Installing {}; earlier reported files may already be updated",
                    path.display()
                )
            })?;
            println!("Updated: {}", path.display());
        }
        println!("Open a new terminal to load PATH changes and completions.");
        Ok(())
    }
}

pub fn install(args: &Args) -> Result<()> {
    if cfg!(windows) {
        bail!("Shell installation supports macOS and Linux; use WSL on Windows");
    }
    let locations = Locations::from_env()?;
    let binary_dir = if args.add_to_path.is_some() {
        let path = match &args.bin_dir {
            Some(path) => std::env::current_dir()?.join(path),
            None => std::env::current_exe()?
                .parent()
                .context("Executable has no parent directory")?
                .to_path_buf(),
        };
        if !path.join("nysos").is_file() {
            bail!(
                "{} must contain a nysos executable; install the binary first",
                path.display()
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(path.join("nysos"))?.permissions().mode() & 0o111 == 0 {
                bail!("{}/nysos is not executable", path.display());
            }
        }
        // Preserve explicit symlink paths (e.g. ~/.nix-profile/bin) across upgrades.
        Some(path)
    } else {
        None
    };
    plan(
        &locations,
        args.add_to_path,
        binary_dir.as_deref(),
        args.install_completions,
    )?
    .apply()
}

fn plan(
    locations: &Locations,
    path_shell: Option<ShellTarget>,
    binary_dir: Option<&Path>,
    completion_shell: Option<ShellTarget>,
) -> Result<Plan> {
    let mut plan = Plan::default();
    if let Some(shell) = path_shell {
        let dir = binary_dir.context("Missing binary directory")?;
        let text = dir
            .to_str()
            .context("Binary directory must be valid UTF-8")?;
        if text.contains(':') {
            bail!("PATH directories cannot contain ':'");
        }
        let quoted = quote(dir)?;
        let body = format!(
            "case \":${{PATH-}}:\" in\n  *:{quoted}:*) ;;\n  *) export PATH={quoted}${{PATH:+:\"$PATH\"}} ;;\nesac"
        );
        if includes(shell, ShellTarget::Bash) {
            for file in locations.bash_files() {
                plan.block(&file, "path", &body)?;
            }
        }
        if includes(shell, ShellTarget::Zsh) {
            for name in [".zprofile", ".zshrc"] {
                plan.block(&locations.zsh.join(name), "path", &body)?;
            }
        }
    }
    if let Some(shell) = completion_shell {
        let dir = locations.data.join("nysos/completions");
        if includes(shell, ShellTarget::Bash) {
            let file = dir.join("nysos.bash");
            plan.script(&file, Shell::Bash)?;
            let quoted = quote(&file)?;
            let body = format!(
                "if [ -n \"${{BASH_VERSION-}}\" ]; then\n  case $- in\n    *i*) [ ! -r {quoted} ] || . {quoted} ;;\n  esac\nfi"
            );
            for file in locations.bash_files() {
                plan.block(&file, "bash-completions", &body)?;
            }
        }
        if includes(shell, ShellTarget::Zsh) {
            let file = dir.join("_nysos");
            plan.script(&file, Shell::Zsh)?;
            let quoted = quote(&file)?;
            // Source the generated function explicitly, so this also works when
            // a framework ran compinit earlier in .zshrc with a cached fpath.
            let body = format!(
                "if [[ -o interactive ]]; then\n  if (( ! $+functions[compdef] )); then\n    autoload -Uz compinit\n    compinit\n  fi\n  if (( $+functions[compdef] )) && [[ -r {quoted} ]]; then\n    source {quoted}\n  fi\nfi"
            );
            plan.block(&locations.zsh.join(".zshrc"), "zsh-completions", &body)?;
        }
    }
    Ok(plan)
}

fn includes(selected: ShellTarget, shell: ShellTarget) -> bool {
    selected == ShellTarget::All || selected == shell
}

fn quote(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .context("Shell installation paths must be valid UTF-8")?;
    if text.contains(['\n', '\r', '\0']) {
        bail!("Shell installation paths cannot contain newlines or NUL");
    }
    Ok(format!("'{}'", text.replace('\'', "'\\''")))
}

fn destination(path: &Path) -> Result<PathBuf> {
    // Edit symlink targets, keeping dotfile-manager links intact. Broken links
    // fail explicitly instead of being replaced with ordinary files.
    if path.is_symlink() {
        return std::fs::canonicalize(path)
            .with_context(|| format!("Resolving {}", path.display()));
    }
    Ok(path.to_owned())
}
fn read_optional(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("Reading {}", path.display())),
    }
}

fn managed_block(existing: &str, name: &str, body: &str) -> Result<String> {
    let start = format!("# >>> nysos {name} >>>");
    let end = format!("# <<< nysos {name} <<<");
    let mut offset = 0;
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for line in existing.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == start {
            starts.push(offset);
        }
        if line.trim_end_matches(['\n', '\r']) == end {
            ends.push(offset + line.len());
        }
        offset += line.len();
    }
    let block = format!("{start}\n{body}\n{end}\n");
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(format!(
            "{existing}{}{block}",
            if existing.is_empty() || existing.ends_with('\n') {
                ""
            } else {
                "\n"
            }
        )),
        ([start], [end]) if start < end => Ok(format!(
            "{}{}{}",
            &existing[..*start],
            block,
            &existing[*end..]
        )),
        _ => bail!("Malformed or duplicate nysos {name} markers; fix the block before installing"),
    }
}

fn write_with_backup(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().context("Destination has no parent")?;
    std::fs::create_dir_all(parent)?;
    let previous = std::fs::metadata(path).ok();
    if let Some(metadata) = &previous {
        let mut backup_name = path
            .file_name()
            .context("Destination has no filename")?
            .to_os_string();
        backup_name.push(".nysos.bak");
        let backup_path = path.with_file_name(backup_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup_path)
        {
            Ok(mut backup) => {
                backup.set_permissions(metadata.permissions())?;
                backup.write_all(&std::fs::read(path)?)?;
                backup.sync_all()?;
                println!("Backup: {}", backup_path.display());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    if let Some(metadata) = previous {
        temp.as_file().set_permissions(metadata.permissions())?;
    }
    temp.write_all(contents.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn locations(root: &Path) -> Locations {
        Locations {
            home: root.join("home"),
            zsh: root.join("zsh config"),
            data: root.join("data with ' quote"),
        }
    }
    #[cfg(unix)]
    #[test]
    fn generated_setup_loads_in_bash_and_zsh_without_duplicate_path() {
        let root = tempfile::tempdir().unwrap();
        let paths = locations(root.path());
        let bin = root.path().join("bin with ' $dollar `tick` [glob]");
        std::fs::create_dir_all(&bin).unwrap();
        plan(
            &paths,
            Some(ShellTarget::All),
            Some(&bin),
            Some(ShellTarget::All),
        )
        .unwrap()
        .apply()
        .unwrap();
        for (variable, fallback, rc, script) in [
            (
                "NYSOS_TEST_BASH",
                "/bin/bash",
                paths.home.join(".bashrc"),
                ". \"$1\"; . \"$1\"; printf '%s\\n' \"$PATH\"; complete -p nysos",
            ),
            (
                "NYSOS_TEST_ZSH",
                "/bin/zsh",
                paths.zsh.join(".zshrc"),
                "source \"$1\"; source \"$1\"; print -r -- \"$PATH\"; print -r -- ${_comps[nysos]}",
            ),
        ] {
            let shell = std::env::var(variable).unwrap_or_else(|_| fallback.into());
            let mut command = std::process::Command::new(shell);
            if variable == "NYSOS_TEST_BASH" {
                command.args(["--noprofile", "--norc"]);
            } else {
                command.arg("-f");
            }
            let output = command
                .args(["-i", "-c", script, "nysos-test"])
                .arg(rc)
                .env("HOME", &paths.home)
                .env("ZDOTDIR", &paths.zsh)
                .env("PATH", "/usr/bin:/bin")
                .output()
                .expect("Shell tests require Bash and Zsh; use nix develop");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert_eq!(
                stdout
                    .lines()
                    .next()
                    .unwrap()
                    .split(':')
                    .filter(|p| *p == bin.to_str().unwrap())
                    .count(),
                1,
                "{stdout}"
            );
            assert!(
                stdout.contains("_nysos"),
                "completion was not registered: {stdout}"
            );
            let completion =
                std::fs::read_to_string(paths.data.join(if variable == "NYSOS_TEST_BASH" {
                    "nysos/completions/nysos.bash"
                } else {
                    "nysos/completions/_nysos"
                }))
                .unwrap();
            assert!(completion.contains("--install-completions"));
        }
    }
    #[test]
    fn repeat_install_preserves_content_backups_and_login_precedence() {
        let root = tempfile::tempdir().unwrap();
        let paths = locations(root.path());
        std::fs::create_dir_all(&paths.home).unwrap();
        let login = paths.home.join(".bash_login");
        std::fs::write(&login, "# keep me\nexport EDITOR=vi\n").unwrap();
        let args = (
            Some(ShellTarget::All),
            Some(Path::new("/tmp/space ' $dollar `tick`")),
            Some(ShellTarget::All),
        );
        plan(&paths, args.0, args.1, args.2)
            .unwrap()
            .apply()
            .unwrap();
        let first = std::fs::read_to_string(&login).unwrap();
        plan(&paths, args.0, args.1, args.2)
            .unwrap()
            .apply()
            .unwrap();
        assert_eq!(first, std::fs::read_to_string(&login).unwrap());
        assert!(first.starts_with("# keep me\nexport EDITOR=vi\n"));
        assert_eq!(
            std::fs::read_to_string(paths.home.join(".bash_login.nysos.bak")).unwrap(),
            "# keep me\nexport EDITOR=vi\n"
        );
        assert!(!paths.home.join(".bash_profile").exists());
        assert!(!paths.home.join(".profile").exists());
        assert!(paths.zsh.join(".zshrc").exists());
        assert!(paths.data.join("nysos/completions/_nysos").exists());
    }
    #[test]
    fn malformed_blocks_fail_and_updates_keep_surrounding_text() {
        assert!(managed_block("# >>> nysos path >>>\noops\n", "path", "new").is_err());
        let once = managed_block("before\n", "path", "old").unwrap() + "after\n";
        let updated = managed_block(&once, "path", "new").unwrap();
        assert!(updated.starts_with("before\n"));
        assert!(updated.ends_with("after\n"));
        assert!(!updated.contains("old"));
    }
    #[cfg(unix)]
    #[test]
    fn preserves_symlinks_and_rejects_broken_ones() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("real");
        let link = root.path().join("link");
        std::fs::write(&target, "# original\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut plan = Plan::default();
        plan.block(&link, "path", "new").unwrap();
        plan.apply().unwrap();
        assert!(link.is_symlink());
        assert!(std::fs::read_to_string(target).unwrap().contains("new"));
        let broken = root.path().join("broken");
        std::os::unix::fs::symlink(root.path().join("missing"), &broken).unwrap();
        assert!(destination(&broken).is_err());
    }
}
