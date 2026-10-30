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
        /// List skills only (omit packs table)
        #[arg(long)]
        skills: bool,
        /// Show skill descriptions
        #[arg(short, long, alias = "desc")]
        description: bool,
        /// Full store report: state, layout, skills, packs and warnings
        #[arg(long)]
        all: bool,
    },
    /// Install skills or packs into this project
    Add {
        /// Skill or pack names to install (leave empty for interactive selection)
        #[arg(value_name = "NAME")]
        skills: Vec<String>,
        /// Interactive multi-selection mode
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Treat the names as packs (needed when a skill and a pack share a name)
        #[arg(long, alias = "packs")]
        pack: bool,
        /// Treat the names as individual skills (needed when a skill and a pack share a name)
        #[arg(long)]
        skill: bool,
        /// Install into global agent directories instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Install into .agents/skills (default)
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Install into .gemini/skills (or ~/.gemini/skills with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
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
    /// Remove skills or packs installed in this project
    Remove {
        /// Skill or pack names to remove
        #[arg(required = true, value_name = "NAME")]
        skills: Vec<String>,
        /// Treat the names as packs (needed when a skill and a pack share a name)
        #[arg(long, alias = "packs")]
        pack: bool,
        /// Treat the names as individual skills (needed when a skill and a pack share a name)
        #[arg(long)]
        skill: bool,
        /// Remove from global agent directories instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Remove from .agents/skills
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Remove from .gemini/skills (or ~/.gemini/skills with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
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
        /// Show status of global skills instead of current project
        #[arg(short = 'g', long)]
        global: bool,
        /// Show status of project skills only
        #[arg(short = 'p', long)]
        project: bool,
    },
    /// Show or manage skill packs in the store
    Pack {
        /// Pack action to run (defaults to `list`)
        #[command(subcommand)]
        action: Option<PackCommand>,
    },
    /// Reconcile the project with the global store
    Sync {
        /// Skill names to sync (syncs all recorded skills when empty)
        #[arg(value_name = "SKILL")]
        skills: Vec<String>,
        /// Push local modifications back to the global store
        #[arg(long)]
        to_store: bool,
        /// Overwrite locally modified copies instead of detaching them (or overwrite diverged store when --to-store)
        #[arg(long)]
        force: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Sync global skills instead of current project
        #[arg(short = 'g', long)]
        global: bool,
    },
    /// List projects managed by skm on this machine
    Projects,
    /// Show or manage AGENTS.md and instruction files
    Rule {
        /// Rule action to run (defaults to `list`)
        #[command(subcommand)]
        action: Option<RuleCommand>,
    },
}

/// Actions of `skm pack`.
///
/// Only `show` and `edit` remain first-class: listing lives in `skm list
/// --packs`, status in `skm status --packs`, and install/remove in `skm add` /
/// `skm remove`. The hidden variants below are deprecated aliases kept for one
/// release; they print a deprecation notice on stderr.
#[derive(Debug, Subcommand)]
pub enum PackCommand {
    /// List packs available in the store (deprecated alias of `skm list --packs`)
    #[command(alias = "ls", hide = true)]
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
        /// Rename the pack to this new name (migrates installed references too)
        #[arg(long, value_name = "NAME")]
        rename: Option<String>,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show status of packs installed in this project (deprecated alias of `skm status --packs`)
    #[command(hide = true)]
    Status {
        /// Show status of global packs instead of current project
        #[arg(short = 'g', long)]
        global: bool,
    },
    /// Install pack(s) into this project (deprecated alias of `skm add [<pack>...]`)
    #[command(hide = true)]
    Add {
        /// Pack names to install (leave empty for interactive selection)
        #[arg(value_name = "PACK")]
        packs: Vec<String>,
        /// Interactive multi-selection mode
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Install into global agent directories instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Install into .agents/skills (default)
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Install into .gemini/skills (or ~/.gemini/skills with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
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
    /// Remove pack(s) from this project (deprecated alias of `skm remove [<pack>...] [--pack]`)
    #[command(hide = true)]
    Remove {
        /// Pack names to remove
        #[arg(required = true, value_name = "PACK")]
        packs: Vec<String>,
        /// Remove from global agent directories instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Remove from .agents/skills
        #[arg(long, alias = "agents")]
        agent: bool,
        /// Remove from .gemini/skills (or ~/.gemini/skills with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
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
    /// Scan the store for skills and report its layout (deprecated alias of `skm list --all`)
    #[command(hide = true)]
    Scan,
}

/// Actions of `skm rule`.
#[derive(Debug, Subcommand)]
pub enum RuleCommand {
    /// List available rules/instructions in the store (same as plain `skm rule`)
    #[command(alias = "ls")]
    List,
    /// Show the content of a rule in the store
    Show {
        /// Rule name to inspect (leave empty or "default" for AGENTS.md)
        #[arg(default_value = "default")]
        name: String,
    },
    /// Apply or update instruction rules in targets
    #[command(alias = "add")]
    Apply {
        /// Rule name in store (defaults to "default" / AGENTS.md)
        name: Option<String>,
        /// Apply to global agent files instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Target .gemini/GEMINI.md (or ~/.gemini/GEMINI.md with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
        /// Target CLAUDE.md (or ~/.claude/CLAUDE.md with --global)
        #[arg(long)]
        claude: bool,
        /// Target AGENTS.md (or ~/.config/opencode/AGENTS.md with --global)
        #[arg(long)]
        opencode: bool,
        /// Target .cursor/AGENTS.md (or ~/.cursor/AGENTS.md with --global)
        #[arg(long)]
        cursor: bool,
        /// Target .pi/AGENTS.md (or ~/.pi/agent/AGENTS.md with --global)
        #[arg(long)]
        pi: bool,
        /// Target .github/copilot-instructions.md (or ~/.copilot/copilot-instructions.md with --global)
        #[arg(long)]
        copilot: bool,
        /// Custom destination file path
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// Interactive selection mode
        #[arg(short = 'i', long)]
        interactive: bool,
        /// Overwrite the entire file with marked instructions if no markers exist
        #[arg(long)]
        overwrite: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Reconcile applied rules with the store
    Sync {
        /// Sync global rules instead of current project
        #[arg(short = 'g', long)]
        global: bool,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show status of applied rules
    Status {
        /// Show status of global rules instead of current project
        #[arg(short = 'g', long)]
        global: bool,
        /// Show status of project rules only
        #[arg(short = 'p', long)]
        project: bool,
    },
    /// Remove applied rule block from target files
    Remove {
        /// Rule name to remove (defaults to "default")
        #[arg(default_value = "default")]
        name: String,
        /// Remove from global agent files instead of project
        #[arg(short = 'g', long)]
        global: bool,
        /// Target .gemini/GEMINI.md (or ~/.gemini/GEMINI.md with --global)
        #[arg(long, alias = "agy")]
        antigravity: bool,
        /// Target CLAUDE.md (or ~/.claude/CLAUDE.md with --global)
        #[arg(long)]
        claude: bool,
        /// Target AGENTS.md (or ~/.config/opencode/AGENTS.md with --global)
        #[arg(long)]
        opencode: bool,
        /// Target .cursor/AGENTS.md (or ~/.cursor/AGENTS.md with --global)
        #[arg(long)]
        cursor: bool,
        /// Target .pi/AGENTS.md (or ~/.pi/agent/AGENTS.md with --global)
        #[arg(long)]
        pi: bool,
        /// Target .github/copilot-instructions.md (or ~/.copilot/copilot-instructions.md with --global)
        #[arg(long)]
        copilot: bool,
        /// Custom destination file path
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
    },
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
                skills: false,
                description: false,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--packs"]).command,
            Commands::List {
                packs: true,
                skills: false,
                description: false,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--skills"]).command,
            Commands::List {
                packs: false,
                skills: true,
                description: false,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--desc"]).command,
            Commands::List {
                packs: false,
                skills: false,
                description: true,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--description"]).command,
            Commands::List {
                packs: false,
                skills: false,
                description: true,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "-d"]).command,
            Commands::List {
                packs: false,
                skills: false,
                description: true,
                all: false,
            }
        ));
        assert!(matches!(
            parse(&["list", "--all"]).command,
            Commands::List {
                packs: false,
                skills: false,
                description: false,
                all: true,
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
            antigravity,
            opencode,
            claude,
            codex,
            pi,
            cursor,
            global,
            dir,
            pin,
            force,
            interactive,
            dry_run,
            skill,
        } = parse(&["add", "alpha", "beta"]).command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha", "beta"]);
        assert!(
            !pack
                && !skill
                && !agent
                && !antigravity
                && !opencode
                && !claude
                && !codex
                && !pi
                && !cursor
                && !global
                && !interactive
        );
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
            interactive,
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
            "--interactive",
            "--dry-run",
        ])
        .command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha"]);
        assert!(pack && agent && opencode && claude && codex && pi && interactive);
        assert_eq!(dir.as_deref(), Some("custom/skills"));
        assert!(pin && force && dry_run);

        let Commands::Add { skill, .. } = parse(&["add", "alpha", "--skill"]).command else {
            panic!("expected `add`");
        };
        assert!(skill);
    }

    #[test]
    fn add_takes_no_store_flag() {
        assert!(Cli::try_parse_from(["skm", "add", "alpha", "--store", "main"]).is_err());
    }

    #[test]
    fn add_supports_interactive_flag_and_empty_skills() {
        let Commands::Add {
            skills,
            interactive,
            ..
        } = parse(&["add", "-i"]).command
        else {
            panic!("expected `add`");
        };
        assert!(skills.is_empty());
        assert!(interactive);

        let Commands::Add {
            skills,
            interactive,
            ..
        } = parse(&["add"]).command
        else {
            panic!("expected `add`");
        };
        assert!(skills.is_empty());
        assert!(!interactive);

        let Commands::Status { project, .. } = parse(&["status", "-p"]).command else {
            panic!("expected `status`");
        };
        assert!(project);
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
            Commands::Status { packs: false, .. }
        ));
        assert!(matches!(
            parse(&["status", "--packs"]).command,
            Commands::Status { packs: true, .. }
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
                action: Some(PackCommand::Status { global: false })
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
            action: Some(PackCommand::Edit {
                name, add, remove, ..
            }),
        } = parse(&["pack", "edit", "frontend", "--add", "tailwind", "seo"]).command
        else {
            panic!("expected `pack edit`");
        };
        assert_eq!(name, "frontend");
        assert_eq!(add, ["tailwind", "seo"]);
        assert!(remove.is_empty());

        let Commands::Pack {
            action: Some(PackCommand::Edit {
                name, add, remove, ..
            }),
        } = parse(&["pack", "edit", "frontend", "--remove", "seo"]).command
        else {
            panic!("expected `pack edit`");
        };
        assert_eq!(name, "frontend");
        assert!(add.is_empty());
        assert_eq!(remove, ["seo"]);

        let Commands::Pack {
            action: Some(PackCommand::Edit { name, rename, .. }),
        } = parse(&["pack", "edit", "frontend", "--rename", "web"]).command
        else {
            panic!("expected `pack edit --rename`");
        };
        assert_eq!(name, "frontend");
        assert_eq!(rename.as_deref(), Some("web"));

        let Commands::Pack {
            action: Some(PackCommand::Edit { rename: None, .. }),
        } = parse(&["pack", "edit", "frontend"]).command
        else {
            panic!("expected `pack edit` without rename");
        };
    }

    #[test]
    fn sync_takes_force_and_dry_run() {
        let Commands::Sync {
            skills,
            to_store,
            force,
            dry_run,
            global,
        } = parse(&["sync"]).command
        else {
            panic!("expected `sync`");
        };
        assert!(skills.is_empty() && !to_store && !force && !dry_run && !global);

        let Commands::Sync {
            skills,
            to_store,
            force,
            dry_run,
            global,
        } = parse(&[
            "sync",
            "alpha",
            "beta",
            "--to-store",
            "--force",
            "--dry-run",
        ])
        .command
        else {
            panic!("expected `sync`");
        };
        assert_eq!(skills, ["alpha", "beta"]);
        assert!(to_store && force && dry_run && !global);
    }

    #[test]
    fn add_and_remove_take_global_and_antigravity_flags() {
        let Commands::Add {
            skills,
            global,
            antigravity,
            ..
        } = parse(&["add", "alpha", "--global", "--antigravity"]).command
        else {
            panic!("expected `add`");
        };
        assert_eq!(skills, ["alpha"]);
        assert!(global);
        assert!(antigravity);

        let Commands::Remove {
            skills,
            global,
            antigravity,
            ..
        } = parse(&["remove", "alpha", "-g", "--agy"]).command
        else {
            panic!("expected `remove`");
        };
        assert_eq!(skills, ["alpha"]);
        assert!(global);
        assert!(antigravity);

        let Commands::Status { global, .. } = parse(&["status", "--global"]).command else {
            panic!("expected `status`");
        };
        assert!(global);

        let Commands::Sync { global, .. } = parse(&["sync", "-g"]).command else {
            panic!("expected `sync`");
        };
        assert!(global);
    }

    #[test]
    fn rule_command_parses_subcommands() {
        let Commands::Rule { action } = parse(&["rule"]).command else {
            panic!("expected `rule`");
        };
        assert!(action.is_none());

        let Commands::Rule {
            action:
                Some(RuleCommand::Apply {
                    name,
                    global,
                    antigravity,
                    pi,
                    copilot,
                    ..
                }),
        } = parse(&[
            "rule",
            "apply",
            "default",
            "-g",
            "--agy",
            "--pi",
            "--copilot",
        ])
        .command
        else {
            panic!("expected `rule apply`");
        };
        assert_eq!(name.as_deref(), Some("default"));
        assert!(global);
        assert!(antigravity);
        assert!(pi);
        assert!(copilot);

        let Commands::Rule {
            action: Some(RuleCommand::Sync { global, dry_run }),
        } = parse(&["rule", "sync", "--dry-run"]).command
        else {
            panic!("expected `rule sync`");
        };
        assert!(!global && dry_run);

        let Commands::Rule {
            action: Some(RuleCommand::Remove { name, global, .. }),
        } = parse(&["rule", "remove", "default", "-g"]).command
        else {
            panic!("expected `rule remove`");
        };
        assert_eq!(name, "default");
        assert!(global);
    }
}
