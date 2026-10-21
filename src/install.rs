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
use crate::pack;
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

/// Target flags specified on the command line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetFlags {
    pub agent: bool,
    pub antigravity: bool,
    pub opencode: bool,
    pub claude: bool,
    pub codex: bool,
    pub pi: bool,
    pub cursor: bool,
    pub global: bool,
}

impl TargetFlags {
    /// Returns `true` if any agent flag was specified.
    pub fn any(self) -> bool {
        self.agent
            || self.antigravity
            || self.opencode
            || self.claude
            || self.codex
            || self.pi
            || self.cursor
    }
}

/// Resolve the install targets for this command.
///
/// If any agent flag or `--dir` is specified, those targets are returned.
/// If no target flag is specified, it defaults to `--agent` (`.agents/skills`, or `config.default_target`).
pub fn resolve_targets(
    config: &Config,
    flags: TargetFlags,
    dir: Option<&str>,
) -> Result<Vec<String>> {
    let mut targets = Vec::new();
    if flags.global {
        if flags.agent {
            targets.push(config.global_agent_target("agent"));
        }
        if flags.antigravity {
            targets.push(config.global_agent_target("antigravity"));
        }
        if flags.opencode {
            targets.push(config.global_agent_target("opencode"));
        }
        if flags.claude {
            targets.push(config.global_agent_target("claude"));
        }
        if flags.codex {
            targets.push(config.global_agent_target("codex"));
        }
        if flags.pi {
            targets.push(config.global_agent_target("pi"));
        }
        if flags.cursor {
            targets.push(config.global_agent_target("cursor"));
        }
        if let Some(dir) = dir {
            if dir.trim().is_empty() {
                bail!("--dir must not be empty");
            }
            targets.push(dir.to_owned());
        }
        if targets.is_empty() {
            targets.push(config.global_agent_target("agent"));
        }
    } else {
        if flags.agent {
            targets.push(config.agent_target("agent"));
        }
        if flags.antigravity {
            targets.push(config.agent_target("antigravity"));
        }
        if flags.opencode {
            targets.push(config.agent_target("opencode"));
        }
        if flags.claude {
            targets.push(config.agent_target("claude"));
        }
        if flags.codex {
            targets.push(config.agent_target("codex"));
        }
        if flags.pi {
            targets.push(config.agent_target("pi"));
        }
        if flags.cursor {
            targets.push(config.agent_target("cursor"));
        }
        if let Some(dir) = dir {
            if dir.trim().is_empty() {
                bail!("--dir must not be empty");
            }
            targets.push(dir.to_owned());
        }
        if targets.is_empty() {
            targets.push(config.agent_target("agent"));
        }
    }
    targets.dedup();
    Ok(targets)
}

/// Resolve targets for `skm remove`.
///
/// If no target flag is given, an empty list is returned (signaling "remove from all targets").
/// If target flags are given, only those specific targets are returned.
pub fn resolve_remove_targets(
    config: &Config,
    flags: TargetFlags,
    dir: Option<&str>,
) -> Result<Vec<String>> {
    if !flags.any() && dir.is_none() {
        return Ok(Vec::new());
    }
    let mut targets = Vec::new();
    if flags.global {
        if flags.agent {
            targets.push(config.global_agent_target("agent"));
        }
        if flags.antigravity {
            targets.push(config.global_agent_target("antigravity"));
        }
        if flags.opencode {
            targets.push(config.global_agent_target("opencode"));
        }
        if flags.claude {
            targets.push(config.global_agent_target("claude"));
        }
        if flags.codex {
            targets.push(config.global_agent_target("codex"));
        }
        if flags.pi {
            targets.push(config.global_agent_target("pi"));
        }
        if flags.cursor {
            targets.push(config.global_agent_target("cursor"));
        }
        if let Some(dir) = dir {
            if dir.trim().is_empty() {
                bail!("--dir must not be empty");
            }
            targets.push(dir.to_owned());
        }
    } else {
        if flags.agent {
            targets.push(config.agent_target("agent"));
        }
        if flags.antigravity {
            targets.push(config.agent_target("antigravity"));
        }
        if flags.opencode {
            targets.push(config.agent_target("opencode"));
        }
        if flags.claude {
            targets.push(config.agent_target("claude"));
        }
        if flags.codex {
            targets.push(config.agent_target("codex"));
        }
        if flags.pi {
            targets.push(config.agent_target("pi"));
        }
        if flags.cursor {
            targets.push(config.agent_target("cursor"));
        }
        if let Some(dir) = dir {
            if dir.trim().is_empty() {
                bail!("--dir must not be empty");
            }
            targets.push(dir.to_owned());
        }
    }
    targets.dedup();
    Ok(targets)
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
        return Ok(config.agent_target(agent));
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
    /// Target directories to install into.
    pub targets: Vec<String>,
    /// Pin the installed copies.
    pub pin: bool,
    /// Overwrite local modifications.
    pub force: bool,
    /// Whether `names` are pack names.
    pub pack: bool,
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
    /// Whether this skill is directly installed.
    pub direct: bool,
    /// Names of packs that include this skill.
    pub pack_names: Vec<String>,
}

/// A planned batch of skills and pack associations to install.
#[derive(Debug, Clone)]
pub struct AddBatchPlan {
    /// Individual skill installations.
    pub plans: Vec<AddPlan>,
    /// Packs to record in the project.
    pub packs: Vec<String>,
}

/// Plan the installation of every named skill or pack, validating before any write.
pub fn plan_add(
    config: &Config,
    project: &Project,
    names: &[String],
    options: &AddOptions,
) -> Result<AddBatchPlan> {
    let targets = if options.targets.is_empty() {
        vec![config.agent_target("agent")]
    } else {
        options.targets.clone()
    };
    let root = project.root();
    let now = registry::now_epoch();

    struct Request {
        skill: String,
        pack: Option<String>,
        direct: bool,
    }

    let mut requests: Vec<Request> = Vec::new();
    let mut recorded_packs: Vec<String> = Vec::new();

    if options.pack {
        let store_root = Path::new(config.store_path()?);
        for pack_name in names {
            let pack_def = pack::load(store_root, pack_name)?;
            if !recorded_packs.contains(&pack_def.name) {
                recorded_packs.push(pack_def.name.clone());
            }
            for skill_name in pack_def.skills {
                requests.push(Request {
                    skill: skill_name,
                    pack: Some(pack_def.name.clone()),
                    direct: false,
                });
            }
        }
    } else {
        for name in names {
            requests.push(Request {
                skill: name.clone(),
                pack: None,
                direct: true,
            });
        }
    }

    let mut unique_skills: Vec<String> = Vec::new();
    for req in &requests {
        if !unique_skills.contains(&req.skill) {
            unique_skills.push(req.skill.clone());
        }
    }

    let mut plans = Vec::new();

    for skill_name in unique_skills {
        let candidate = candidate(config, &skill_name)?;
        let skill_requests: Vec<_> = requests.iter().filter(|r| r.skill == skill_name).collect();
        let is_direct_req = skill_requests.iter().any(|r| r.direct);
        let req_packs: Vec<String> = skill_requests
            .iter()
            .filter_map(|r| r.pack.clone())
            .collect();

        for target in &targets {
            let dest_dir = target_dir(root, target).join(&candidate.skill.name);
            let dest_rel = if target.starts_with('~') {
                format!("{target}/{}", candidate.skill.name)
            } else {
                skill::relative_path(&dest_dir, root)
            };
            let existing = project.record.get(&candidate.skill.name, target);

            let dest_exists = dest_dir.exists();
            let action = match existing {
                Some(entry) if dest_exists => {
                    let current = hash::tree_hash(&dest_dir)?;
                    if current != entry.written_hash && !options.force {
                        bail!(
                            "skill '{skill_name}' was modified in {dest_rel}; pass --force to overwrite it or run `skm remove {skill_name}`"
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

            let direct = is_direct_req || existing.is_some_and(|e| e.direct);
            let mut pack_names = existing.map(|e| e.packs.clone()).unwrap_or_default();
            for p in req_packs.clone() {
                if !pack_names.contains(&p) {
                    pack_names.push(p);
                }
            }
            pack_names.sort();

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
                direct,
                pack_names,
            });
        }
    }

    Ok(AddBatchPlan {
        plans,
        packs: recorded_packs,
    })
}

/// Copy every planned skill and record each in the central project registry.
pub fn apply_add(project: &mut Project, batch: &AddBatchPlan) -> Result<()> {
    for plan in &batch.plans {
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
            direct: plan.direct,
            packs: plan.pack_names.clone(),
            ..SkillState::default()
        });
    }

    for pack in &batch.packs {
        project.record.add_pack(pack);
    }

    project.save()
}

/// What `skm remove` does with one declared skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveAction {
    /// Deletes the copies and the entries.
    Remove,
    /// Retains the copies on disk because of remaining references.
    Retain { reason: String },
}

impl RemoveAction {
    /// Verb of a real run.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Remove => "Removed",
            Self::Retain { .. } => "Retained",
        }
    }

    /// Verb of a `--dry-run`.
    pub fn dry_verb(&self) -> &'static str {
        match self {
            Self::Remove => "Would remove",
            Self::Retain { .. } => "Would retain",
        }
    }
}

/// Options of `skm remove`.
#[derive(Debug, Clone, Default)]
pub struct RemoveOptions {
    /// Specific targets to remove from (empty means all targets).
    pub targets: Vec<String>,
    /// Delete copies that were modified.
    pub force: bool,
    /// Whether `names` are pack names.
    pub pack: bool,
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

/// One skill about to be uninstalled or retained.
#[derive(Debug, Clone)]
pub struct RemovePlan {
    /// Skill name.
    pub skill: String,
    /// Copies affected.
    pub copies: Vec<InstalledCopy>,
    /// What will be done.
    pub action: RemoveAction,
    /// If retaining, the new `direct` flag.
    pub new_direct: Option<bool>,
    /// If retaining, the remaining packs.
    pub remaining_packs: Option<Vec<String>>,
}

/// A planned batch of removals and pack unlinks.
#[derive(Debug, Clone)]
pub struct RemoveBatchPlan {
    /// Individual skill removals or retain actions.
    pub plans: Vec<RemovePlan>,
    /// Packs to remove from the project record.
    pub packs: Vec<String>,
}

/// Plan the removal of every named skill or pack, validating before any write.
pub fn plan_remove(
    config: &Config,
    project: &Project,
    names: &[String],
    options: &RemoveOptions,
) -> Result<RemoveBatchPlan> {
    config.store_path()?;
    let root = project.root();
    let mut plans = Vec::new();
    let mut recorded_packs = Vec::new();

    if options.pack {
        let mut packs_to_remove = Vec::new();
        for pack_name in names {
            let has_pack = project.record.has_pack(pack_name);
            let has_skills = project
                .record
                .skills
                .iter()
                .any(|s| s.packs.iter().any(|p| p == pack_name));
            if !has_pack && !has_skills {
                bail!("pack '{pack_name}' is not registered in this project");
            }
            if !packs_to_remove.contains(pack_name) {
                packs_to_remove.push(pack_name.clone());
            }
        }
        recorded_packs = packs_to_remove.clone();

        // Find all skills associated with ANY of packs_to_remove
        let mut affected_skills: Vec<String> = Vec::new();
        for skill_state in &project.record.skills {
            if skill_state
                .packs
                .iter()
                .any(|p| packs_to_remove.contains(p))
                && !affected_skills.contains(&skill_state.name)
            {
                affected_skills.push(skill_state.name.clone());
            }
        }
        affected_skills.sort();

        for skill_name in affected_skills {
            let mut entries = project.record.get_all(&skill_name);
            if !options.targets.is_empty() {
                entries.retain(|e| options.targets.contains(&e.target));
            }
            if entries.is_empty() {
                continue;
            }

            let mut copies = Vec::new();
            for entry in &entries {
                let dest_dir = installed_dir(root, &entry.target, &skill_name);
                let dest_rel = if entry.target.starts_with('~') {
                    format!("{}/{}", entry.target, skill_name)
                } else {
                    skill::relative_path(&dest_dir, root)
                };
                copies.push(InstalledCopy {
                    target: entry.target.clone(),
                    dest_rel,
                    dest_dir,
                });
            }

            let mut any_direct = false;
            let mut remaining_packs: Vec<String> = Vec::new();
            for entry in &entries {
                if entry.direct {
                    any_direct = true;
                }
                for p in &entry.packs {
                    if !packs_to_remove.contains(p) && !remaining_packs.contains(p) {
                        remaining_packs.push(p.clone());
                    }
                }
            }
            remaining_packs.sort();

            if any_direct || !remaining_packs.is_empty() {
                let reason = if any_direct {
                    "still required by direct install".to_owned()
                } else {
                    format!("still required by pack '{}'", remaining_packs.join("', '"))
                };
                plans.push(RemovePlan {
                    skill: skill_name,
                    copies,
                    action: RemoveAction::Retain { reason },
                    new_direct: None,
                    remaining_packs: Some(remaining_packs),
                });
            } else {
                for copy in &copies {
                    if copy.dest_dir.is_dir() {
                        let entry = entries.iter().find(|e| e.target == copy.target).unwrap();
                        let current = hash::tree_hash(&copy.dest_dir)?;
                        if current != entry.written_hash && !options.force {
                            bail!(
                                "skill '{skill_name}' was modified in {}; pass --force to delete it anyway",
                                copy.dest_rel
                            );
                        }
                    }
                }
                plans.push(RemovePlan {
                    skill: skill_name,
                    copies,
                    action: RemoveAction::Remove,
                    new_direct: None,
                    remaining_packs: None,
                });
            }
        }
    } else {
        let mut seen: Vec<&str> = Vec::new();
        for name in names {
            if seen.contains(&name.as_str()) {
                continue;
            }
            seen.push(name);

            let mut entries = project.record.get_all(name);
            if !options.targets.is_empty() {
                entries.retain(|e| options.targets.contains(&e.target));
            }

            if entries.is_empty() {
                bail!("skill '{name}' is not registered in this project");
            }

            let mut copies = Vec::new();
            for entry in &entries {
                let dest_dir = installed_dir(root, &entry.target, name);
                let dest_rel = if entry.target.starts_with('~') {
                    format!("{}/{}", entry.target, name)
                } else {
                    skill::relative_path(&dest_dir, root)
                };
                copies.push(InstalledCopy {
                    target: entry.target.clone(),
                    dest_rel,
                    dest_dir,
                });
            }

            let mut all_packs: Vec<String> = Vec::new();
            for entry in &entries {
                for p in &entry.packs {
                    if !all_packs.contains(p) {
                        all_packs.push(p.clone());
                    }
                }
            }

            if !all_packs.is_empty() && !options.force {
                let any_direct = entries.iter().any(|e| e.direct);
                if any_direct {
                    let reason = format!("still required by pack '{}'", all_packs.join("', '"));
                    plans.push(RemovePlan {
                        skill: name.clone(),
                        copies,
                        action: RemoveAction::Retain { reason },
                        new_direct: Some(false),
                        remaining_packs: None,
                    });
                    continue;
                } else {
                    bail!(
                        "skill '{name}' is part of pack '{}'; pass --force to delete it anyway or run `skm pack remove {}`",
                        all_packs.join("', '"),
                        all_packs[0]
                    );
                }
            }

            for copy in &copies {
                if copy.dest_dir.is_dir() {
                    let entry = entries.iter().find(|e| e.target == copy.target).unwrap();
                    let current = hash::tree_hash(&copy.dest_dir)?;
                    if current != entry.written_hash && !options.force {
                        bail!(
                            "skill '{name}' was modified in {}; pass --force to delete it anyway",
                            copy.dest_rel
                        );
                    }
                }
            }

            plans.push(RemovePlan {
                skill: name.clone(),
                copies,
                action: RemoveAction::Remove,
                new_direct: None,
                remaining_packs: None,
            });
        }
    }

    Ok(RemoveBatchPlan {
        plans,
        packs: recorded_packs,
    })
}

/// Delete every planned copy or unlink pack references in the central registry.
pub fn apply_remove(project: &mut Project, batch: &RemoveBatchPlan) -> Result<()> {
    if batch.plans.is_empty() && batch.packs.is_empty() {
        return Ok(());
    }

    for plan in &batch.plans {
        match &plan.action {
            RemoveAction::Remove => {
                for copy in &plan.copies {
                    fsops::remove_path(&copy.dest_dir)
                        .with_context(|| format!("removing {}", copy.dest_dir.display()))?;
                    project.record.remove(&plan.skill, &copy.target);
                }
            }
            RemoveAction::Retain { .. } => {
                for copy in &plan.copies {
                    if let Some(entry) = project.record.get_mut(&plan.skill, &copy.target) {
                        if let Some(direct) = plan.new_direct {
                            entry.direct = direct;
                        }
                        if let Some(packs) = &plan.remaining_packs {
                            entry.packs = packs.clone();
                        }
                    }
                }
            }
        }
    }

    for pack in &batch.packs {
        let still_in_use = project
            .record
            .skills
            .iter()
            .any(|s| s.packs.iter().any(|p| p == pack));
        if !still_in_use {
            project.record.remove_pack(pack);
        }
    }

    project.save()
}
