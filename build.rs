// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::Path, process::Command};

fn output(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    if !result.status.success() {
        return None;
    }
    let text = String::from_utf8(result.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.lines().collect::<Vec<_>>().join(", "))
}

fn git(args: &[&str]) -> Option<String> {
    output("git", args)
}

fn main() {
    for path in ["build.rs", "src", "examples", "Cargo.toml", "Cargo.lock"] {
        println!("cargo:rerun-if-changed={path}");
    }
    for name in [
        "NYSOS_GIT_COMMIT",
        "NYSOS_GIT_TAG",
        "NYSOS_GIT_DESCRIBE",
        "RUSTC",
        "SOURCE_DATE_EPOCH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // Archives inside another checkout must not inherit that checkout's identity.
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let own_checkout = git(&["rev-parse", "--show-toplevel"]).is_some_and(|top| {
        Path::new(&top).canonicalize().ok() == Path::new(&root).canonicalize().ok()
    });
    let supplied = env::var_os("NYSOS_GIT_COMMIT").is_some();
    if own_checkout && !supplied {
        // Resolve both paths: worktrees store HEAD locally and refs in the common dir.
        for option in ["--git-dir", "--git-common-dir"] {
            if let Some(dir) = git(&["rev-parse", option]) {
                for entry in ["HEAD", "refs", "packed-refs", "logs/HEAD"] {
                    // Watch the nearest existing parent for refs not created yet.
                    // Cargo treats a nonexistent watched path as always changed.
                    let mut path = Path::new(&dir).join(entry);
                    while !path.exists() && path.pop() {}
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
    let metadata = |name: &str, args: &[&str]| {
        env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| (own_checkout && !supplied).then(|| git(args)).flatten())
            .unwrap_or_else(|| "unknown".into())
            .lines()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let commit = metadata("NYSOS_GIT_COMMIT", &["rev-parse", "--verify", "HEAD"]);
    let tag = metadata("NYSOS_GIT_TAG", &["tag", "--points-at", "HEAD"]);
    let describe = metadata(
        "NYSOS_GIT_DESCRIBE",
        &["describe", "--tags", "--always", "--abbrev=12"],
    );
    let rustc = output(
        &env::var("RUSTC").unwrap_or_else(|_| "rustc".into()),
        &["--version"],
    )
    .unwrap_or_else(|| "unknown".into());
    let value = |name| env::var(name).unwrap_or_else(|_| "unknown".into());
    let (built_at, timestamp_source) = match env::var("SOURCE_DATE_EPOCH") {
        Ok(epoch) => {
            let seconds = epoch
                .parse::<i64>()
                .ok()
                .filter(|seconds| *seconds >= 0)
                .expect("SOURCE_DATE_EPOCH must be nonnegative Unix seconds");
            (
                time::OffsetDateTime::from_unix_timestamp(seconds)
                    .expect("SOURCE_DATE_EPOCH is outside the supported calendar range"),
                " (SOURCE_DATE_EPOCH)",
            )
        }
        Err(env::VarError::NotPresent) => (time::OffsetDateTime::now_utc(), ""),
        Err(error) => panic!("Invalid SOURCE_DATE_EPOCH: {error}"),
    };
    let build_time = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z{timestamp_source}",
        built_at.year(),
        built_at.month() as u8,
        built_at.day(),
        built_at.hour(),
        built_at.minute(),
        built_at.second()
    );
    let report = format!(
        "{}\nBuild time (UTC): {build_time}\nGit commit: {commit}\nGit tag: {tag}\nGit describe: {describe}\nCompiler: {rustc}\nTarget: {}\nProfile: {} (opt-level {})",
        value("CARGO_PKG_VERSION"),
        value("TARGET"),
        value("PROFILE"),
        value("OPT_LEVEL")
    );
    // A generated Rust string preserves newlines without emitting Cargo directives.
    std::fs::write(
        Path::new(&value("OUT_DIR")).join("version.rs"),
        format!("{report:?}"),
    )
    .unwrap();
}
