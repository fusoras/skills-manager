//! `skm add` and `skm remove`: real copies of skills in and out of a project.
//!
//! Every command is planned first ([`plan_add`] / [`plan_remove`]) and applied
//! afterwards ([`apply_add`] / [`apply_remove`]), so validation failures never
//! leave the project half modified.
//!
//! Projects remain 100% clean: metadata is persisted exclusively in the
//! central registry (`~/.local/state/skm/projects.toml`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::fsops;
use crate::hash;
use crate::project::Project;
use crate::registry::{self, SkillState};
use crate::skill::{self, Skill};
use crate::store;

/// A skill the global store provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Store root, resolved from the global configuration.
    pub store_root: PathBuf,
    /// The skill itself.
    pub skill: Skill,
}

impl Candidate {
    /// Directory that holds the skill inside the store.
    pub fn source_dir(&self) -> PathBuf {
        self.store_root.join(&self.skill.rel_path)
    }
}

/// The skill `name` in the global store.
pub fn candidate(config: &Config, name: &str) -> Result<Candidate> {
    let report = store::scan(config.store_path()?)?;
    if !report.ready {
        bail!("{}", report.warnings.join("; "));
    }
    let skill = report
        .skills
        .into_iter()
        .find(|skill| skill.name == name)
        .with_context(|| {
            format!(
                "skill '{name}' not found in the store; \
                 run `skm list` to see the available skills"
            )
        })?;
    Ok(Candidate {
        store_root: report.resolved,
        skill,
    })
}

/// Resolve the install target for this command.
///
/// `--dir` wins over `--agent`, which wins over the project's recorded target,
/// which wins over the global `default_target`.
pub fn resolve_target(
    config: &Config,
    project: &Project,
    dir: Option<&str>,
    agent: Option<&str>,
) -> Result<String> {
    if let Some(dir) = dir {
        if dir.trim().is_empty() {
            bail!("--dir must not be empty");
        }
        return Ok(dir.to_owned());
    }
    if let Some(agent) = agent {
        return config.agents.get(agent).cloned().with_context(|| {
            format!("agent '{agent}' is not defined in the global config ([agents] table)")
        });
    }
    if let Some(target) = project.record.target.as_deref()
        && !target.trim().is_empty()
    {
        return Ok(target.to_owned());
    }
    Ok(config.default_target.clone())
}

/// Directory a target string points at; relative targets are project relative.
pub fn target_dir(root: &Path, target: &str) -> PathBuf {
    let expanded = store::expand_tilde(target);
    if expanded.is_absolute() {
        expanded
    } else {
        root.join(expanded)
    }
}

/// Directory of one installed copy.
pub fn installed_dir(root: &Path, target: &str, name: &str) -> PathBuf {
    target_dir(root, target).join(name)
}

/// What `skm add` does with the destination directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing was installed there yet.
    Install,
    /// Already installed and untouched.
    Resync,
    /// Overwrites changed content, or content `skm` does not manage.
    Replace,
}

impl Action {
    /// Verb of a real run.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Install => "Installed",
            Self::Resync => "Re-synced",
            Self::Replace => "Replaced",
        }
    }

    /// Verb of a `--dry-run`.
    pub fn dry_verb(self) -> &'static str {
        match self {
            Self::Install => "Would install",
            Self::Resync => "Would re-sync",
            Self::Replace => "Would replace",
        }
    }
}

/// Options of `skm add`.
#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    /// Explicit target directory.
    pub dir: Option<String>,
    /// Target directory of a configured agent.
    pub agent: Option<String>,
    /// Pin the installed copies.
    pub pin: bool,
    /// Overwrite local modifications.
    pub force: bool,
}

/// One skill about to be installed.
#[derive(Debug, Clone)]
pub struct AddPlan {
    /// Skill name.
    pub skill: String,
    /// Directory of the skill in the store.
    pub source_dir: PathBuf,
    /// Skill directory relative to the store root.
    pub source_rel: String,
    /// Target directory, as resolved.
    pub target: String,
    /// Destination directory of this skill.
    pub dest_dir: PathBuf,
    /// Destination as shown to the user.
    pub dest_rel: String,
    /// What will be done with the destination.
    pub action: Action,
    /// Pin the installed copy.
    pub pinned: bool,
    /// Timestamp to record.
    pub installed_at: i64,
}

/// Plan the installation of every named skill, validating before any write.
pub fn plan_add(
    config: &Config,
    project: &Project,
    names: &[String],
    options: &AddOptions,
) -> Result<Vec<AddPlan>> {
    let target = resolve_target(
        config,
        project,
        options.dir.as_deref(),
        options.agent.as_deref(),
    )?;
    let root = project.root();
    let now = registry::now_epoch();
    let mut plans = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for name in names {
        if seen.contains(&name.as_str()) {
            continue;
        }
        seen.push(name);

        let candidate = candidate(config, name)?;
        let dest_dir = target_dir(root, &target).join(&candidate.skill.name);
        let dest_rel = skill::relative_path(&dest_dir, root);
        let existing = project
            .record
            .get(&candidate.skill.name, &target)
            .filter(|entry| !entry.detached);

        let dest_exists = dest_dir.exists();
        let action = match existing {
            Some(entry) if dest_exists => {
                let current = hash::tree_hash(&dest_dir)?;
                if current != entry.written_hash && !options.force {
                    bail!(
                        "skill '{name}' was modified in {dest_rel}; pass --force to overwrite it or run `skm remove {name}`"
                    );
                }
                if current != entry.written_hash {
                    Action::Replace
                } else {
                    Action::Resync
                }
            }
            None if dest_exists && !options.force => bail!(
                "{dest_rel} already exists but is not managed by skm; pass --force to overwrite it"
            ),
            None if dest_exists => Action::Replace,
            _ => Action::Install,
        };

        let pinned = options.pin || existing.is_some_and(|entry| entry.pinned);
        let installed_at = match (action, existing) {
            (Action::Resync, Some(entry)) => entry.installed_at,
            _ => now,
        };

        plans.push(AddPlan {
            skill: candidate.skill.name.clone(),
            source_dir: candidate.source_dir(),
            source_rel: candidate.skill.rel_path.clone(),
            target: target.clone(),
            dest_dir,
            dest_rel,
            action,
            pinned,
            installed_at,
        });
    }
    Ok(plans)
}

/// Copy every planned skill and record each in the central project registry.
pub fn apply_add(project: &mut Project, plans: &[AddPlan]) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    for plan in plans {
        fsops::install_tree(&plan.source_dir, &plan.dest_dir)
            .with_context(|| format!("installing skill '{}'", plan.skill))?;

        project.record.upsert(SkillState {
            name: plan.skill.clone(),
            source_rel: plan.source_rel.clone(),
            target: plan.target.clone(),
            written_hash: hash::tree_hash(&plan.dest_dir)?,
            source_hash: hash::tree_hash(&plan.source_dir)?,
            pinned: plan.pinned,
            installed_at: plan.installed_at,
            ..SkillState::default()
        });
    }
    project.save()
}

/// What `skm remove` does with one declared skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveAction {
    /// Deletes the copies and the entries.
    Remove,
}

impl RemoveAction {
    /// Verb of a real run.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Remove => "Removed",
        }
    }

    /// Verb of a `--dry-run`.
    pub fn dry_verb(self) -> &'static str {
        match self {
            Self::Remove => "Would remove",
        }
    }
}

/// Options of `skm remove`.
#[derive(Debug, Clone, Default)]
pub struct RemoveOptions {
    /// Delete copies that were modified.
    pub force: bool,
}

/// One installed copy on disk.
#[derive(Debug, Clone)]
pub struct InstalledCopy {
    /// Target directory, as recorded in the registry.
    pub target: String,
    /// Directory of the copy.
    pub dest_dir: PathBuf,
    /// Copy as shown to the user.
    pub dest_rel: String,
}

/// One skill about to be uninstalled.
#[derive(Debug, Clone)]
pub struct RemovePlan {
    /// Skill name.
    pub skill: String,
    /// Copies to delete.
    pub copies: Vec<InstalledCopy>,
    /// What will be done.
    pub action: RemoveAction,
}

/// Plan the removal of every named skill, validating before any write.
pub fn plan_remove(
    config: &Config,
    project: &Project,
    names: &[String],
    options: &RemoveOptions,
) -> Result<Vec<RemovePlan>> {
    config.store_path()?;

    let root = project.root();
    let mut plans = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for name in names {
        if seen.contains(&name.as_str()) {
            continue;
        }
        seen.push(name);

        let entries = project.record.get_all(name);

        if entries.is_empty() {
            bail!("skill '{name}' is not registered in this project");
        }

        let mut copies = Vec::new();
        for entry in entries {
            let dest_dir = installed_dir(root, &entry.target, name);
            if dest_dir.is_dir() {
                let current = hash::tree_hash(&dest_dir)?;
                if current != entry.written_hash && !options.force {
                    bail!(
                        "skill '{name}' was modified in {}; pass --force to delete it anyway",
                        skill::relative_path(&dest_dir, root)
                    );
                }
            }
            copies.push(InstalledCopy {
                target: entry.target.clone(),
                dest_rel: skill::relative_path(&dest_dir, root),
                dest_dir,
            });
        }
        plans.push(RemovePlan {
            skill: name.clone(),
            copies,
            action: RemoveAction::Remove,
        });
    }
    Ok(plans)
}

/// Delete every planned copy, then drop the entries from the central registry.
pub fn apply_remove(project: &mut Project, plans: &[RemovePlan]) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    for plan in plans {
        for copy in &plan.copies {
            fsops::remove_path(&copy.dest_dir)
                .with_context(|| format!("removing {}", copy.dest_dir.display()))?;
            project.record.remove(&plan.skill, &copy.target);
        }
    }
    project.save()
}
