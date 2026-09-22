// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

#[path = "../src/cli.rs"]
mod cli;
#[path = "../src/prefix.rs"]
#[allow(dead_code)]
mod prefix;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::{io::Write, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let destination = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("output directory required"),
    );
    std::fs::create_dir_all(&destination)?;
    for (shell, name) in [(Shell::Bash, "nysos.bash"), (Shell::Zsh, "_nysos")] {
        let mut file = std::fs::File::create(destination.join(name))?;
        writeln!(
            file,
            "# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)\n# SPDX-License-Identifier: Apache-2.0"
        )?;
        generate(shell, &mut cli::Args::command(), "nysos", &mut file);
    }
    Ok(())
}
