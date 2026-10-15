//! `skm status`: what a project declares, what is installed and what changed.
//!
//! Status is strictly read only: it hashes directories to classify them, but
//! never writes a hash, a file or a directory.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::hash;
use crate::install::{installed_dir, resolve_target, target_dir};
use crate::project::Project;
use crate::state::SkillState;
use crate::targets;

/// Health of one copy, from the most to the least urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Installed, untouched, and the store still holds the same content.
    Ok,
    /// The copy in the project differs from what `skm` wrote.
    Modified,
    /// The store content differs from the content that was installed.
    Behind,
    /// In sync and pinned, so later syncs leave it alone.
    Pinned,
    /// Declared or recorded, but absent from disk.
    Missing,
    /// A directory in a target that neither the manifest nor the state knows.
    Unmanaged,
    /// A local fork: the declaration was dropped and sync never touches it.
    Detached,
    /// In sync locally, but with no registered store to compare against.
    Skipped,
}

impl Health {
    /// Label used in the STATE column.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Modified => "modified",
            Self::Behind => "behind",
            Self::Pinned => "pinned",
            Self::Missing => "missing",
            Self::Unmanaged => "unmanaged",
            Self::Detached => "detached",
            Self::Skipped => "skipped",
        }
    }

    /// Parenthesized note that explains the label, if any.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Skipped => Some("no store registered"),
            _ => None,
        }
    }
}

/// One row of `skm status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRow {
    /// Skill name.
    pub skill: String,
    /// Target directory, as recorded or as resolved from the defaults.
    pub target: String,
    /// Health of that copy.
    pub health: Health,
}

/// Every declared skill plus every unmanaged directory, sorted by skill.
pub fn rows(config: &Config, project: &Project) -> Result<Vec<StatusRow>> {
    let root = project.root();
    let default_target = resolve_target(config, project, None, None)?;
    let store_root = targets::store_root(config);
    let mut rows: Vec<StatusRow> = Vec::new();

    // Declared skills, plus state entries the manifest no longer knows about.
    let mut subjects: Vec<(&str, bool)> = Vec::new();
    for install in &project.manifest.install {
        if let Some(name) = install.skill.as_deref() {
            subjects.push((name, install.pin));
        }
    }
    for entry in &project.state.skills {
        let known = subjects.iter().any(|(name, _)| *name == entry.name);
        if !known {
            subjects.push((&entry.name, entry.pinned));
        }
    }

    for (name, declared_pin) in subjects {
        for target in targets::state_targets(project, name, &default_target) {
            let dest_dir = installed_dir(root, &target, name);
            let entry = project.state.get(name, &target);
            // Pinned either in the manifest (declaration) or in the state (install).
            let pinned = declared_pin || entry.is_some_and(|entry| entry.pinned);
            let health = health_of(store_root.as_deref(), &dest_dir, entry, pinned)?;
            rows.push(StatusRow {
                skill: name.to_owned(),
                target,
                health,
            });
        }
    }

    for target in targets::known_targets(project, &default_target) {
        let dir = target_dir(root, &target);
        if !dir.is_dir() {
            continue;
        }
        for name in targets::subdirectories(&dir)? {
            let declared = rows
                .iter()
                .any(|row| row.skill == name && row.target == target);
            if !declared {
                rows.push(StatusRow {
                    skill: name,
                    target: target.clone(),
                    health: Health::Unmanaged,
                });
            }
        }
    }

    rows.sort_by(|left, right| {
        left.skill
            .cmp(&right.skill)
            .then_with(|| left.target.cmp(&right.target))
    });
    Ok(rows)
}

/// Health of one copy of a skill.
///
/// The local checks (missing, modified, detached, unmanaged) need nothing but
/// the project; only `behind`, `pinned` and `ok` compare with the store, so a
/// copy that passes every local check is `skipped` when no store is registered.
fn health_of(
    store_root: Option<&Path>,
    dest_dir: &Path,
    entry: Option<&SkillState>,
    pinned: bool,
) -> Result<Health> {
    if !dest_dir.is_dir() {
        return Ok(Health::Missing);
    }
    let Some(entry) = entry else {
        // The directory is there, but `skm` never wrote it.
        return Ok(Health::Unmanaged);
    };
    if entry.detached {
        // A fork: never compared with the store again.
        return Ok(Health::Detached);
    }
    if hash::tree_hash(dest_dir)? != entry.written_hash {
        return Ok(Health::Modified);
    }
    let Some(store_root) = store_root else {
        return Ok(Health::Skipped);
    };
    if source_changed(store_root, entry)? {
        return Ok(Health::Behind);
    }
    Ok(if pinned { Health::Pinned } else { Health::Ok })
}

/// `true` when the store content differs from the recorded source hash.
fn source_changed(store_root: &Path, entry: &SkillState) -> Result<bool> {
    let source = store_root.join(&entry.source_rel);
    if !source.is_dir() {
        // The skill disappeared from the store.
        return Ok(true);
    }
    Ok(hash::tree_hash(&source)? != entry.source_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    use crate::fsops;
    use crate::project;
    use crate::state::{self, State};
    use tempfile::TempDir;

    /// A store with one skill and an empty project.
    struct Fixture {
        _tmp: TempDir,
        store: PathBuf,
        project: PathBuf,
        config: Config,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_store(true)
        }

        /// [`Fixture::new`] with the store registered in the config or not.
        fn with_store(registered: bool) -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let store = tmp.path().join("store");
            fs::create_dir_all(store.join("alpha")).unwrap();
            fs::write(store.join("alpha").join("SKILL.md"), "name: alpha\n").unwrap();

            let project = tmp.path().join("project");
            fs::create_dir_all(&project).unwrap();

            let config = Config {
                store: registered.then(|| store.display().to_string()),
                ..Config::default()
            };

            Self {
                _tmp: tmp,
                store,
                project,
                config,
            }
        }

        /// Manifest declaring `alpha`.
        fn manifest(&self) -> String {
            "[[install]]\nskill = \"alpha\"\n".to_owned()
        }

        /// Copy the skill of the store into the project, as `skm add` would.
        fn install(&self, target: &str, pinned: bool) -> SkillState {
            let dest = self.project.join(target).join("alpha");
            fsops::copy_tree(&self.store.join("alpha"), &dest).unwrap();
            SkillState {
                name: "alpha".to_owned(),
                source_rel: "alpha".to_owned(),
                target: target.to_owned(),
                written_hash: hash::tree_hash(&dest).unwrap(),
                source_hash: hash::tree_hash(&self.store.join("alpha")).unwrap(),
                pinned,
                installed_at: 1_760_536_800,
                ..SkillState::default()
            }
        }

        /// Open the project with `manifest` and `entries` written to disk.
        fn open(&self, manifest: &str, entries: &[SkillState]) -> Project {
            fs::write(project::manifest_path(&self.project), manifest).unwrap();
            if !entries.is_empty() {
                let mut state = State::default();
                for entry in entries {
                    state.upsert(entry.clone());
                }
                state::save(&project::state_path(&self.project), &state).unwrap();
            }
            Project::open_at(&self.project).unwrap()
        }

        fn rows(&self, manifest: &str, entries: &[SkillState]) -> Vec<StatusRow> {
            let project = self.open(manifest, entries);
            rows(&self.config, &project).unwrap()
        }

        fn dest(&self) -> PathBuf {
            self.project.join(".agents").join("skills").join("alpha")
        }
    }

    #[test]
    fn declared_without_a_copy_is_missing() {
        let fixture = Fixture::new();
        let rows = fixture.rows(&fixture.manifest(), &[]);

        assert_eq!(
            rows,
            [StatusRow {
                skill: "alpha".to_owned(),
                target: ".agents/skills".to_owned(),
                health: Health::Missing,
            }]
        );
    }

    #[test]
    fn an_untouched_copy_is_ok() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        let rows = fixture.rows(&fixture.manifest(), &[entry]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].health, Health::Ok);
    }

    #[test]
    fn an_edited_copy_is_modified() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        fs::write(fixture.dest().join("extra.md"), "local work\n").unwrap();

        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].health, Health::Modified);
    }

    #[test]
    fn a_changed_store_is_behind() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        fs::write(
            fixture.store.join("alpha").join("SKILL.md"),
            "name: alpha\nv2\n",
        )
        .unwrap();

        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].health, Health::Behind);
    }

    #[test]
    fn a_vanished_store_skill_is_behind() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        fs::remove_dir_all(fixture.store.join("alpha")).unwrap();

        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].health, Health::Behind);
    }

    #[test]
    fn a_pinned_copy_is_reported_as_pinned_unless_it_changed() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", true);

        let rows = fixture.rows(&fixture.manifest(), std::slice::from_ref(&entry));
        assert_eq!(rows[0].health, Health::Pinned);

        // A local edit is more urgent than the pin, and so is a store update.
        fs::write(fixture.dest().join("extra.md"), "local work\n").unwrap();
        let rows = fixture.rows(&fixture.manifest(), std::slice::from_ref(&entry));
        assert_eq!(rows[0].health, Health::Modified);

        fs::remove_file(fixture.dest().join("extra.md")).unwrap();
        fs::write(
            fixture.store.join("alpha").join("SKILL.md"),
            "name: alpha\nv2\n",
        )
        .unwrap();
        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].health, Health::Behind);
    }

    #[test]
    fn the_manifest_pin_flag_marks_a_copy_as_pinned() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        let manifest = format!("{}pin = true\n", fixture.manifest());

        let rows = fixture.rows(&manifest, &[entry]);
        assert_eq!(rows[0].health, Health::Pinned);
    }

    #[test]
    fn directories_nothing_declares_are_unmanaged() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.project.join(".agents/skills/rogue")).unwrap();
        fs::create_dir_all(fixture.project.join(".agents/skills/.git")).unwrap();

        let rows = fixture.rows("", &[]);

        assert_eq!(
            rows,
            [StatusRow {
                skill: "rogue".to_owned(),
                target: ".agents/skills".to_owned(),
                health: Health::Unmanaged,
            }]
        );
    }

    #[test]
    fn a_directory_skm_did_not_write_is_unmanaged() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);
        // Same name, but a copy that was never recorded in the state.
        let rows = fixture.rows(&fixture.manifest(), &[]);
        assert_eq!(rows[0].health, Health::Unmanaged);
        assert_eq!(entry.target, ".agents/skills");
    }

    #[test]
    fn state_entries_the_manifest_lost_are_still_reported() {
        let fixture = Fixture::new();
        let entry = fixture.install(".agents/skills", false);

        let rows = fixture.rows("", std::slice::from_ref(&entry));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].skill, "alpha");
        assert_eq!(rows[0].target, ".agents/skills");
        assert_eq!(rows[0].health, Health::Ok);
    }

    #[test]
    fn without_a_store_only_the_vs_store_verdict_is_skipped() {
        let fixture = Fixture::with_store(false);
        let entry = fixture.install(".agents/skills", false);

        // Locally untouched: the missing comparison is the only unknown.
        let rows = fixture.rows(&fixture.manifest(), std::slice::from_ref(&entry));
        assert_eq!(rows[0].health, Health::Skipped);
        assert_eq!(rows[0].health.note(), Some("no store registered"));

        // Local verdicts do not need the store and are still reported.
        fs::write(fixture.dest().join("extra.md"), "local work\n").unwrap();
        let rows = fixture.rows(&fixture.manifest(), std::slice::from_ref(&entry));
        assert_eq!(rows[0].health, Health::Modified);

        fs::remove_dir_all(fixture.dest()).unwrap();
        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].health, Health::Missing);
    }

    #[test]
    fn an_explicit_target_from_the_state_is_used() {
        let fixture = Fixture::new();
        let entry = fixture.install(".claude/skills", false);

        let rows = fixture.rows(&fixture.manifest(), &[entry]);
        assert_eq!(rows[0].target, ".claude/skills");
        assert_eq!(rows[0].health, Health::Ok);
    }

    #[test]
    fn a_detached_fork_is_reported_as_detached() {
        let fixture = Fixture::new();
        let entry = SkillState {
            content_hash: Some("blake3:local".to_owned()),
            detached_at: Some(1_760_600_000),
            detached: true,
            ..fixture.install(".agents/skills", false)
        };
        // The fork is a local edit of the installed copy.
        fs::write(fixture.dest().join("local.md"), "local work\n").unwrap();

        let rows = fixture.rows("", std::slice::from_ref(&entry));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].skill, "alpha");
        assert_eq!(rows[0].target, ".agents/skills");
        assert_eq!(rows[0].health, Health::Detached);

        // A branch that disappeared is missing, not detached.
        fs::remove_dir_all(fixture.dest()).unwrap();
        let rows = fixture.rows("", std::slice::from_ref(&entry));
        assert_eq!(rows[0].health, Health::Missing);
    }

    #[test]
    fn labels_are_the_documented_words() {
        assert_eq!(Health::Ok.label(), "ok");
        assert_eq!(Health::Modified.label(), "modified");
        assert_eq!(Health::Behind.label(), "behind");
        assert_eq!(Health::Pinned.label(), "pinned");
        assert_eq!(Health::Missing.label(), "missing");
        assert_eq!(Health::Unmanaged.label(), "unmanaged");
        assert_eq!(Health::Detached.label(), "detached");
        assert_eq!(Health::Skipped.label(), "skipped");
        assert_eq!(Health::Skipped.note(), Some("no store registered"));
        assert_eq!(Health::Ok.note(), None);
    }
}
