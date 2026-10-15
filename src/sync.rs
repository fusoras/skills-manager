//! `skm sync`: reconcile the copies of a project with the global store.
//!
//! Sync is planned first ([`plan_sync`]) and applied afterwards ([`apply_sync`]),
//! so a validation failure never leaves the project half updated. For every
//! declaration it installs a missing copy, refreshes an untouched one whose store
//! changed, adopts a copy that already matches the store, and detaches a locally
//! modified one: the declaration is dropped from the manifest and a tombstone is
//! written to the local state, so later syncs leave the fork alone for good.
//!
//! Nothing else is ever touched: pinned copies are never updated (only restored
//! when they are missing), and directories that nothing declares are reported as
//! `unmanaged` and left exactly as they are.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::fsops;
use crate::hash;
use crate::install::{installed_dir, resolve_target, target_dir};
use crate::manifest;
use crate::project::Project;
use crate::skill;
use crate::state::{self, SkillState};
use crate::store;
use crate::targets;

/// What `skm sync` does with one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// The copy and the store are the ones recorded.
    Ok,
    /// The store changed and the copy was refreshed from it.
    Updated,
    /// The copy was missing and has been written.
    Installed,
    /// A pinned copy was missing and has been written back.
    Restored,
    /// A copy that already matched the store was recorded.
    Adopted,
    /// A locally modified copy was overwritten because `--force` was given.
    Replaced,
    /// The copy exists and is pinned: never updated.
    Pinned,
    /// A locally modified copy: the declaration was dropped, the fork kept.
    Detached,
    /// The store cannot provide the skill.
    Skipped,
    /// A directory in a target that nothing declares.
    Unmanaged,
}

impl SyncAction {
    /// Every action, in the order the summary counts them.
    pub const ALL: [Self; 10] = [
        Self::Ok,
        Self::Updated,
        Self::Installed,
        Self::Restored,
        Self::Adopted,
        Self::Replaced,
        Self::Pinned,
        Self::Detached,
        Self::Skipped,
        Self::Unmanaged,
    ];

    /// Keyword that starts the output line.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Updated => "updated",
            Self::Installed => "installed",
            Self::Restored => "restored",
            Self::Adopted => "adopted",
            Self::Replaced => "replaced",
            Self::Pinned => "pinned",
            Self::Detached => "detached",
            Self::Skipped => "skipped",
            Self::Unmanaged => "unmanaged",
        }
    }

    /// Parenthesized note that closes the output line, if any.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Replaced => Some("forced"),
            Self::Detached => Some("local modifications"),
            Self::Skipped => Some(SkipReason::Unavailable.note()),
            Self::Unmanaged => Some("not managed by skm"),
            Self::Ok
            | Self::Updated
            | Self::Installed
            | Self::Restored
            | Self::Adopted
            | Self::Pinned => None,
        }
    }

    /// `true` for the actions that rewrite the copy from the store.
    fn writes_a_copy(self) -> bool {
        matches!(
            self,
            Self::Installed | Self::Restored | Self::Updated | Self::Replaced
        )
    }
}

/// Why a row was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// No store is registered in the global configuration.
    NoStore,
    /// The store cannot provide the skill.
    Unavailable,
}

impl SkipReason {
    /// Note shown after the `skipped` keyword.
    pub fn note(self) -> &'static str {
        match self {
            Self::NoStore => "no store registered",
            Self::Unavailable => "store unavailable",
        }
    }
}

/// One reconciliation step.
#[derive(Debug, Clone)]
pub struct SyncPlan {
    /// Skill name; for `unmanaged` rows, the directory name.
    pub skill: String,
    /// Target directory, as recorded or as resolved from the defaults.
    pub target: String,
    /// Copy to write, for the actions that install one.
    pub source_dir: Option<PathBuf>,
    /// Skill directory relative to the store root.
    pub source_rel: String,
    /// Copy of the skill in the project.
    pub dest_dir: PathBuf,
    /// Copy as shown to the user.
    pub dest_rel: String,
    /// What will be done.
    pub action: SyncAction,
    /// Why the row was skipped; `None` for every other action.
    pub skip: Option<SkipReason>,
    /// State entry the action records, when it changes the state.
    pub entry: Option<SkillState>,
}

impl SyncPlan {
    /// A plan for the copy of `skill` in `target`, without a decision yet.
    fn new(skill: &str, target: &str, root: &Path, action: SyncAction) -> Self {
        let dest_dir = installed_dir(root, target, skill);
        Self {
            skill: skill.to_owned(),
            target: target.to_owned(),
            source_dir: None,
            source_rel: String::new(),
            dest_rel: skill::relative_path(&dest_dir, root),
            dest_dir,
            action,
            skip: None,
            entry: None,
        }
    }

    /// Parenthesized note of this row, if any.
    pub fn note(&self) -> Option<&'static str> {
        match self.action {
            SyncAction::Skipped => Some(self.skip.unwrap_or(SkipReason::Unavailable).note()),
            action => action.note(),
        }
    }
}

/// The directory of a skill inside the store, with its store relative path.
struct Source {
    /// Directory of the skill in the store.
    dir: PathBuf,
    /// Skill directory relative to the store root, `/` separated.
    rel: String,
}

/// Plan the reconciliation of every declaration, validating before any write.
pub fn plan_sync(config: &Config, project: &Project, force: bool) -> Result<Vec<SyncPlan>> {
    let root = project.root();
    let default_target = resolve_target(config, project, None, None)?;
    let now = state::now_epoch();
    let mut plans: Vec<SyncPlan> = Vec::new();

    for install in &project.manifest.install {
        // Pack entries are rejected before planning; only skills arrive here.
        let Some(name) = install.skill.as_deref() else {
            continue;
        };

        for target in targets::state_targets(project, name, &default_target) {
            // A tombstone is a local fork, not an install: never re-adopted.
            let entry = project
                .state
                .get(name, &target)
                .filter(|entry| !entry.detached);
            let pinned = install.pin || entry.is_some_and(|entry| entry.pinned);
            let dest_dir = installed_dir(root, &target, name);

            if pinned && dest_dir.is_dir() {
                // Pinned copies are never updated, not even to follow the store.
                plans.push(SyncPlan::new(name, &target, root, SyncAction::Pinned));
                continue;
            }

            let mut plan = SyncPlan::new(name, &target, root, SyncAction::Skipped);
            let Some(source) = find_source(config, name, entry)? else {
                // Without a store nothing can be reconciled; a store that
                // cannot provide the skill is reported the same way.
                plan.skip = Some(if config.store.is_some() {
                    SkipReason::Unavailable
                } else {
                    SkipReason::NoStore
                });
                plans.push(plan);
                continue;
            };
            let source_hash = hash::tree_hash(&source.dir)?;
            let current = dest_dir
                .is_dir()
                .then(|| hash::tree_hash(&dest_dir))
                .transpose()?;

            plan.action = match current.as_deref() {
                // Nothing on disk: write it back, pinned or not.
                None if pinned => SyncAction::Restored,
                None => SyncAction::Installed,
                // A copy skm never wrote: adopt it when it matches the store.
                Some(current) => match entry {
                    None if current == source_hash => SyncAction::Adopted,
                    None if force => SyncAction::Replaced,
                    None => SyncAction::Detached,
                    Some(entry) if current != entry.written_hash && !force => SyncAction::Detached,
                    Some(entry) if current != entry.written_hash => SyncAction::Replaced,
                    Some(entry) if source_hash == entry.source_hash => SyncAction::Ok,
                    Some(_) => SyncAction::Updated,
                },
            };
            plan.source_rel = source.rel;
            if plan.action.writes_a_copy() {
                plan.source_dir = Some(source.dir);
            }
            plan.entry = match plan.action {
                SyncAction::Installed
                | SyncAction::Restored
                | SyncAction::Updated
                | SyncAction::Adopted
                | SyncAction::Replaced => Some(SkillState {
                    name: name.to_owned(),
                    source_rel: plan.source_rel.clone(),
                    target: target.clone(),
                    written_hash: source_hash.clone(),
                    source_hash,
                    pinned,
                    // An existing install keeps its date; only a new one is dated now.
                    installed_at: entry.map_or(now, |entry| entry.installed_at),
                    ..SkillState::default()
                }),
                // A fork: keep the files as they are and record what is there.
                SyncAction::Detached => {
                    let base = match entry {
                        // Re-detaching a recorded copy: the last install stays on record.
                        Some(entry) => entry.clone(),
                        // A copy skm never wrote: the found content is all there is.
                        None => SkillState {
                            name: name.to_owned(),
                            source_rel: plan.source_rel.clone(),
                            target: target.clone(),
                            written_hash: current.clone().unwrap_or_default(),
                            source_hash: source_hash.clone(),
                            pinned,
                            installed_at: now,
                            ..SkillState::default()
                        },
                    };
                    Some(SkillState {
                        content_hash: current.clone(),
                        detached_at: Some(now),
                        detached: true,
                        ..base
                    })
                }
                _ => None,
            };
            plans.push(plan);
        }
    }

    // Directories nothing declares nor records are reported, never touched.
    let known: Vec<(&str, &str)> = project
        .state
        .skills
        .iter()
        .map(|entry| (entry.name.as_str(), entry.target.as_str()))
        .collect();
    for target in targets::known_targets(project, &default_target) {
        let dir = target_dir(root, &target);
        if !dir.is_dir() {
            continue;
        }
        for name in targets::subdirectories(&dir)? {
            let covered = known
                .iter()
                .any(|(known, known_target)| *known == name.as_str() && *known_target == target)
                || plans
                    .iter()
                    .any(|plan| plan.skill == name && plan.target == target);
            if !covered {
                plans.push(SyncPlan::new(&name, &target, root, SyncAction::Unmanaged));
            }
        }
    }

    plans.sort_by(|left, right| {
        left.skill
            .cmp(&right.skill)
            .then_with(|| left.target.cmp(&right.target))
    });
    Ok(plans)
}

/// Resolve `name` in the global store: the recorded path first, a scan as fallback.
///
/// The recorded path keeps the common case cheap; the scan finds a skill that
/// moved inside the store instead of reporting it as unavailable.
fn find_source(config: &Config, name: &str, entry: Option<&SkillState>) -> Result<Option<Source>> {
    let Some(path_as_written) = config.store.as_deref() else {
        return Ok(None);
    };
    let root = store::expand_tilde(path_as_written);
    if let Some(rel) = entry.map(|entry| entry.source_rel.as_str()) {
        let dir = root.join(rel);
        if skill::is_skill_dir(&dir) {
            return Ok(Some(Source {
                dir,
                rel: rel.to_owned(),
            }));
        }
    }

    Ok(store::scan(path_as_written)?
        .skills
        .into_iter()
        .find(|skill| skill.name == name)
        .map(|skill| Source {
            dir: root.join(&skill.rel_path),
            rel: skill.rel_path,
        }))
}

/// Write every planned copy, refresh the state, drop the detached declarations.
pub fn apply_sync(project: &mut Project, plans: &[SyncPlan]) -> Result<()> {
    let mut document = project.document()?;
    let mut manifest_changed = false;

    for plan in plans {
        if let Some(source) = &plan.source_dir {
            fsops::install_tree(source, &plan.dest_dir)
                .with_context(|| format!("installing skill '{}'", plan.skill))?;
        }
        if let Some(entry) = &plan.entry {
            project.state.upsert(entry.clone());
            // Saved per skill so an interrupted run still leaves a truthful state.
            project.save_state()?;
        }
        if plan.action == SyncAction::Detached
            && manifest::doc_remove_install(&mut document, &plan.skill) > 0
        {
            manifest_changed = true;
        }
    }

    if manifest_changed {
        project.write_document(&document)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
