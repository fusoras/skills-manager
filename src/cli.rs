//! Command line interface definition.

use clap::{Parser, Subcommand};

/// Manage skills (directories with a `SKILL.md`) from the global store into projects.
#[derive(Debug, Parser)]
#[command(name = "skm", version, propagate_version = true)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Show or inspect the global configuration file
    Config {
        /// Configuration action to run.
        #[command(subcommand)]
        action: ConfigCommand,
    },
    /// Show or manage the global skill store
    Store {
        /// Store action to run (defaults to `show`)
        #[command(subcommand)]
        action: Option<StoreCommand>,
    },
    /// List skills available in the store
    List,
    /// Install skills into this project
    Add {
        /// Skill names to install
        #[arg(required = true, value_name = "SKILL")]
        skills: Vec<String>,
        /// Install into the target directory of this agent
        #[arg(long, value_name = "NAME")]
        agent: Option<String>,
        /// Install into this directory instead of the resolved target
        #[arg(long, value_name = "PATH")]
        dir: Option<String>,
        /// Pin the copies, so later syncs leave them alone
        #[arg(long)]
        pin: bool,
        /// Overwrite local modifications
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove skills installed in this project
    Remove {
        /// Skill names to remove
        #[arg(required = true, value_name = "SKILL")]
        skills: Vec<String>,
        /// Delete copies that were modified
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the state of every skill declared in this project
    Status,
    /// Reconcile the project with the global store
    Sync {
        /// Overwrite locally modified copies instead of detaching them
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// List projects managed by skm on this machine
    Projects,
}

/// Actions of `skm config`.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the path of the global configuration file
    Path,
    /// Print the current configuration as TOML
    Show,
}

/// Actions of `skm store`.
#[derive(Debug, Subcommand)]
pub enum StoreCommand {
    /// Register the store, replacing the current one
    Add {
        /// Store root directory; may use '~'
        path: String,
        /// Create the directory when it does not exist
        #[arg(long)]
        create: bool,
    },
    /// Show the registered store (same as plain `skm store`)
    Show,
    /// Unregister the store
    Remove,
    /// Scan the store for skills and report its layout
    Scan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["skm"];
        full.extend_from_slice(args);
        Cli::try_parse_from(full).unwrap()
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn store_without_a_subcommand_shows_the_store() {
        let Commands::Store { action } = parse(&["store"]).command else {
            panic!("expected `store`");
        };
        assert!(action.is_none());
    }

    #[test]
    fn store_add_takes_a_path_and_the_create_flag() {
        let Commands::Store {
            action: Some(StoreCommand::Add { path, create }),
        } = parse(&["store", "add", "~/store"]).command
        else {
            panic!("expected `store add`");
        };
        assert_eq!(path, "~/store");
        assert!(!create);

        let Commands::Store {
            action: Some(StoreCommand::Add { create, .. }),
        } = parse(&["store", "add", "~/store", "--create"]).command
        else {
            panic!("expected `store add`");
        };
        assert!(create);
    }

    #[test]
    fn store_add_requires_the_path() {
        assert!(Cli::try_parse_from(["skm", "store", "add"]).is_err());
    }

    #[test]
    fn store_show_remove_and_scan_take_no_arguments() {
        assert!(matches!(
            parse(&["store", "show"]).command,
            Commands::Store {
                action: Some(StoreCommand::Show)
            }
        ));
        assert!(matches!(
            parse(&["store", "remove"]).command,
            Commands::Store {
                action: Some(StoreCommand::Remove)
            }
        ));
        assert!(matches!(
            parse(&["store", "scan"]).command,
            Commands::Store {
                action: Some(StoreCommand::Scan)
            }
        ));
        assert!(Cli::try_parse_from(["skm", "store", "scan", "main"]).is_err());
        assert!(Cli::try_parse_from(["skm", "store", "remove", "main"]).is_err());
    }

    #[test]
    fn list_takes_no_arguments() {
        assert!(matches!(parse(&["list"]).command, Commands::List));
        assert!(Cli::try_parse_from(["skm", "list", "--store", "main"]).is_err());
    }

    #[test]
    fn config_has_path_and_show() {
        assert!(matches!(
            parse(&["config", "path"]).command,
            Commands::Config {
                action: ConfigCommand::Path
            }
        ));
        assert!(matches!(
            parse(&["config", "show"]).command,
            Commands::Config {
                action: ConfigCommand::Show
            }
        ));
    }

    #[test]
    fn add_takes_skills_and_optional_flags() {
        let Commands::Add {
            skills,
            agent,
            dir,
            pin,
            force,
            dry_run,
        } = parse(&["add", "alpha", "beta"]).command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha", "beta"]);
        assert!(agent.is_none() && dir.is_none());
        assert!(!pin && !force && !dry_run);

        let Commands::Add {
            skills,
            agent,
            dir,
            pin,
            force,
            dry_run,
        } = parse(&[
            "add",
            "alpha",
            "--agent",
            "opencode",
            "--dir",
            "custom/skills",
            "--pin",
            "--force",
            "--dry-run",
        ])
        .command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha"]);
        assert_eq!(agent.as_deref(), Some("opencode"));
        assert_eq!(dir.as_deref(), Some("custom/skills"));
        assert!(pin && force && dry_run);
    }

    #[test]
    fn add_takes_no_store_flag() {
        assert!(Cli::try_parse_from(["skm", "add", "alpha", "--store", "main"]).is_err());
    }

    #[test]
    fn add_and_remove_require_at_least_one_skill() {
        assert!(Cli::try_parse_from(["skm", "add"]).is_err());
        assert!(Cli::try_parse_from(["skm", "remove"]).is_err());
    }

    #[test]
    fn remove_takes_skills_and_flags() {
        let Commands::Remove {
            skills,
            force,
            dry_run,
        } = parse(&["remove", "alpha"]).command
        else {
            panic!("expected `remove`");
        };
        assert_eq!(skills, ["alpha"]);
        assert!(!force && !dry_run);

        let Commands::Remove { force, dry_run, .. } =
            parse(&["remove", "alpha", "--force", "--dry-run"]).command
        else {
            panic!("expected `remove`");
        };
        assert!(force && dry_run);
        assert!(Cli::try_parse_from(["skm", "remove", "alpha", "--store", "main"]).is_err());
    }

    #[test]
    fn status_has_no_arguments() {
        assert!(matches!(parse(&["status"]).command, Commands::Status));
    }

    #[test]
    fn sync_takes_force_and_dry_run() {
        let Commands::Sync { force, dry_run } = parse(&["sync"]).command else {
            panic!("expected `sync`");
        };
        assert!(!force && !dry_run);

        let Commands::Sync { force, dry_run } = parse(&["sync", "--force", "--dry-run"]).command
        else {
            panic!("expected `sync`");
        };
        assert!(force && dry_run);
    }
}
