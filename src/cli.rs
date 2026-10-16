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
    List {
        /// List packs instead of individual skills
        #[arg(long)]
        packs: bool,
        /// Show skill descriptions
        #[arg(short, long, alias = "desc")]
        description: bool,
    },
    /// Install skills into this project
    Add {
        /// Skill names to install
        #[arg(required = true, value_name = "SKILL")]
        skills: Vec<String>,
        /// Install a pack of skills instead of individual skills
        #[arg(long, alias = "packs")]
        pack: bool,
        /// Install into .agents/skills (default)
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Install into .opencode/skills
        #[arg(long)]
        opencode: bool,
        /// Install into .claude/skills
        #[arg(long)]
        claude: bool,
        /// Install into .codex/skills
        #[arg(long)]
        codex: bool,
        /// Install into .pi/skills
        #[arg(long)]
        pi: bool,
        /// Install into .cursor/skills
        #[arg(long)]
        cursor: bool,
        /// Install into this custom directory
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
        /// Remove a pack of skills instead of individual skills
        #[arg(long, alias = "packs")]
        pack: bool,
        /// Remove from .agents/skills
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Remove from .opencode/skills
        #[arg(long)]
        opencode: bool,
        /// Remove from .claude/skills
        #[arg(long)]
        claude: bool,
        /// Remove from .codex/skills
        #[arg(long)]
        codex: bool,
        /// Remove from .pi/skills
        #[arg(long)]
        pi: bool,
        /// Remove from .cursor/skills
        #[arg(long)]
        cursor: bool,
        /// Remove from this custom directory
        #[arg(long, value_name = "PATH")]
        dir: Option<String>,
        /// Delete copies that were modified
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the state of every skill declared in this project
    Status {
        /// Show status of packs instead of individual skills
        #[arg(long)]
        packs: bool,
    },
    /// Show or manage skill packs in the store
    Pack {
        /// Pack action to run (defaults to `list`)
        #[command(subcommand)]
        action: Option<PackCommand>,
    },
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

/// Actions of `skm pack`.
#[derive(Debug, Subcommand)]
pub enum PackCommand {
    /// List packs available in the store (same as plain `skm pack`)
    #[command(alias = "ls")]
    List,
    /// Show details of a pack in the store
    Show {
        /// Pack name to inspect
        name: String,
    },
    /// Edit a pack in $EDITOR or modify its skills directly with --add / --remove
    Edit {
        /// Pack name to edit
        name: String,
        /// Add skill(s) to this pack
        #[arg(long, num_args = 1.., value_name = "SKILL")]
        add: Vec<String>,
        /// Remove skill(s) from this pack
        #[arg(long, num_args = 1.., value_name = "SKILL")]
        remove: Vec<String>,
    },
    /// Show status of packs installed in this project
    Status,
    /// Install pack(s) into this project
    Add {
        /// Pack names to install
        #[arg(required = true, value_name = "PACK")]
        packs: Vec<String>,
        /// Install into .agents/skills (default)
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Install into .opencode/skills
        #[arg(long)]
        opencode: bool,
        /// Install into .claude/skills
        #[arg(long)]
        claude: bool,
        /// Install into .codex/skills
        #[arg(long)]
        codex: bool,
        /// Install into .pi/skills
        #[arg(long)]
        pi: bool,
        /// Install into .cursor/skills
        #[arg(long)]
        cursor: bool,
        /// Install into this custom directory
        #[arg(long, value_name = "PATH")]
        dir: Option<String>,
        /// Pin the copies
        #[arg(long)]
        pin: bool,
        /// Overwrite local modifications
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove pack(s) from this project
    Remove {
        /// Pack names to remove
        #[arg(required = true, value_name = "PACK")]
        packs: Vec<String>,
        /// Remove from .agents/skills
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Remove from .opencode/skills
        #[arg(long)]
        opencode: bool,
        /// Remove from .claude/skills
        #[arg(long)]
        claude: bool,
        /// Remove from .codex/skills
        #[arg(long)]
        codex: bool,
        /// Remove from .pi/skills
        #[arg(long)]
        pi: bool,
        /// Remove from .cursor/skills
        #[arg(long)]
        cursor: bool,
        /// Remove from this custom directory
        #[arg(long, value_name = "PATH")]
        dir: Option<String>,
        /// Delete copies that were modified
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
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
    fn list_takes_optional_packs_flag() {
        assert!(matches!(
            parse(&["list"]).command,
            Commands::List {
                packs: false,
                description: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--packs"]).command,
            Commands::List {
                packs: true,
                description: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--desc"]).command,
            Commands::List {
                packs: false,
                description: true,
            }
        ));
        assert!(matches!(
            parse(&["list", "--description"]).command,
            Commands::List {
                packs: false,
                description: true,
            }
        ));
        assert!(matches!(
            parse(&["list", "-d"]).command,
            Commands::List {
                packs: false,
                description: true,
            }
        ));
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
            pack,
            agent,
            opencode,
            claude,
            codex,
            pi,
            cursor,
            dir,
            pin,
            force,
            dry_run,
        } = parse(&["add", "alpha", "beta"]).command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha", "beta"]);
        assert!(!pack && !agent && !opencode && !claude && !codex && !pi && !cursor);
        assert!(dir.is_none());
        assert!(!pin && !force && !dry_run);

        let Commands::Add {
            skills,
            pack,
            agent,
            opencode,
            claude,
            codex,
            pi,
            dir,
            pin,
            force,
            dry_run,
            ..
        } = parse(&[
            "add",
            "alpha",
            "--pack",
            "--agent",
            "--opencode",
            "--claude",
            "--codex",
            "--pi",
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
        assert!(pack && agent && opencode && claude && codex && pi);
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
            pack,
            force,
            dry_run,
            ..
        } = parse(&["remove", "alpha"]).command
        else {
            panic!("expected `remove`");
        };
        assert_eq!(skills, ["alpha"]);
        assert!(!pack && !force && !dry_run);

        let Commands::Remove {
            pack,
            force,
            dry_run,
            ..
        } = parse(&["remove", "alpha", "--pack", "--force", "--dry-run"]).command
        else {
            panic!("expected `remove`");
        };
        assert!(pack && force && dry_run);
        assert!(Cli::try_parse_from(["skm", "remove", "alpha", "--store", "main"]).is_err());
    }

    #[test]
    fn status_takes_optional_packs_flag() {
        assert!(matches!(
            parse(&["status"]).command,
            Commands::Status { packs: false }
        ));
        assert!(matches!(
            parse(&["status", "--packs"]).command,
            Commands::Status { packs: true }
        ));
    }

    #[test]
    fn pack_command_parses_subcommands() {
        assert!(matches!(
            parse(&["pack"]).command,
            Commands::Pack { action: None }
        ));
        assert!(matches!(
            parse(&["pack", "list"]).command,
            Commands::Pack {
                action: Some(PackCommand::List)
            }
        ));
        assert!(matches!(
            parse(&["pack", "status"]).command,
            Commands::Pack {
                action: Some(PackCommand::Status)
            }
        ));
        let Commands::Pack {
            action: Some(PackCommand::Show { name }),
        } = parse(&["pack", "show", "frontend"]).command
        else {
            panic!("expected `pack show`");
        };
        assert_eq!(name, "frontend");

        let Commands::Pack {
            action: Some(PackCommand::Add { packs, agent, .. }),
        } = parse(&["pack", "add", "frontend", "--agent"]).command
        else {
            panic!("expected `pack add`");
        };
        assert_eq!(packs, ["frontend"]);
        assert!(agent);

        let Commands::Pack {
            action: Some(PackCommand::Edit { name, add, remove }),
        } = parse(&["pack", "edit", "frontend", "--add", "tailwind", "seo"]).command
        else {
            panic!("expected `pack edit`");
        };
        assert_eq!(name, "frontend");
        assert_eq!(add, ["tailwind", "seo"]);
        assert!(remove.is_empty());

        let Commands::Pack {
            action: Some(PackCommand::Edit { name, add, remove }),
        } = parse(&["pack", "edit", "frontend", "--remove", "seo"]).command
        else {
            panic!("expected `pack edit`");
        };
        assert_eq!(name, "frontend");
        assert!(add.is_empty());
        assert_eq!(remove, ["seo"]);
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
