//! `skm` - manage skills (directories with a `SKILL.md`) from local stores.
//!
//! Stores skills from a local store into projects with ZERO pollution in target projects:
//! no `skills.toml`, no `.skills-manager/` directory. All project metadata is tracked
//! centrally in the user state directory (`~/.local/state/skm/projects.toml`).

mod cli;
mod commands;
mod config;
mod doc;
mod fsops;
mod hash;
mod install;
mod interactive;
mod output;
mod pack;
mod project;
mod registry;
mod skill;
mod status;
mod store;
mod sync;
mod targets;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match commands::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        // `skm ... | head` closes stdout early; that is not worth an error.
        Err(error) if output::is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
