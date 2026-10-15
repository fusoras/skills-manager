//! `skm` - manage skills (directories with a `SKILL.md`) from local stores.
//!
//! M0 covers the global configuration and the `store` commands; M1 adds
//! `add` / `remove` / `status`, which copy skills into a project as real files
//! and record what was written in `.skills-manager/state.toml`; M2 adds `sync`,
//! which reconciles those copies with the stores, keeps pinned ones untouched and
//! detaches locally modified ones. Packs and watch mode come later.

mod cli;
mod commands;
mod config;
mod doc;
mod fsops;
mod hash;
mod install;
mod manifest;
mod output;
mod project;
mod skill;
mod state;
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
