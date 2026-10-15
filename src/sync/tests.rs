//! Unit tests of the sync planner and applier.

use super::*;
use std::fs;
use tempfile::TempDir;

/// A store with one skill, a project, and the config that binds them.
struct Fixture {
    _tmp: TempDir,
    store: PathBuf,
    root: PathBuf,
    registry_path: PathBuf,
    config: Config,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store");
        fs::create_dir_all(store.join("alpha")).unwrap();
        fs::write(store.join("alpha").join("SKILL.md"), "name: alpha\nv1\n").unwrap();

        let root = tmp.path().join("project");
        fs::create_dir_all(&root).unwrap();

        let registry_path = tmp.path().join("state").join("projects.toml");

        let config = Config {
            store: Some(store.display().to_string()),
            default_target: ".agents/skills".to_owned(),
            ..Config::default()
        };

        Self {
            _tmp: tmp,
            store,
            root,
            registry_path,
            config,
        }
    }

    fn dest(&self) -> PathBuf {
        self.root.join(".agents").join("skills").join("alpha")
    }

    fn source_hash(&self) -> String {
        hash::tree_hash(&self.store.join("alpha")).unwrap()
    }

    /// Copy the skill into the project, as `skm add` would, and record it.
    fn entry(&self, pinned: bool) -> SkillState {
        let dest = self.dest();
        fsops::copy_tree(&self.store.join("alpha"), &dest).unwrap();
        SkillState {
            name: "alpha".to_owned(),
            source_rel: "alpha".to_owned(),
            target: ".agents/skills".to_owned(),
            written_hash: hash::tree_hash(&dest).unwrap(),
            source_hash: self.source_hash(),
            pinned,
            installed_at: 1_760_536_800,
            ..SkillState::default()
        }
    }

    fn open(&self, entries: &[SkillState]) -> Project {
        let mut project = Project::open_at_with(&self.root, &self.registry_path).unwrap();
        for entry in entries {
            project.record.upsert(entry.clone());
        }
        project.save().unwrap();
        project
    }

    fn plan(&self, entries: &[SkillState], force: bool) -> Vec<SyncPlan> {
        let project = self.open(entries);
        plan_sync(&self.config, &project, force).unwrap()
    }

    /// Plan and apply, as the command does, and return the plans.
    fn sync(&self, entries: &[SkillState], force: bool) -> Vec<SyncPlan> {
        let mut project = self.open(entries);
        let plans = plan_sync(&self.config, &project, force).unwrap();
        apply_sync(&mut project, &plans).unwrap();
        plans
    }

    /// Read the project back after a sync.
    fn reopen(&self) -> Project {
        Project::open_at_with(&self.root, &self.registry_path).unwrap()
    }

    fn recorded(&self, name: &str) -> SkillState {
        self.reopen()
            .record
            .get(name, ".agents/skills")
            .cloned()
            .expect("the entry was recorded")
    }

    fn read_dest(&self, file: &str) -> String {
        fs::read_to_string(self.dest().join(file)).unwrap()
    }
}

#[test]
fn keywords_and_notes_are_the_documented_words() {
    assert_eq!(SyncAction::ALL.len(), 10);
    let labels: Vec<(&str, Option<&str>)> = SyncAction::ALL
        .iter()
        .map(|action| (action.keyword(), action.note()))
        .collect();

    assert_eq!(
        labels,
        [
            ("ok", None),
            ("updated", None),
            ("installed", None),
            ("restored", None),
            ("adopted", None),
            ("replaced", Some("forced")),
            ("pinned", None),
            ("detached", Some("local modifications")),
            ("skipped", Some("store unavailable")),
            ("unmanaged", Some("not managed by skm")),
        ]
    );
    assert_eq!(SkipReason::Unavailable.note(), "store unavailable");
    assert_eq!(SkipReason::NoStore.note(), "no store registered");
}

#[test]
fn an_untouched_copy_is_ok_and_writes_nothing() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    let mut project = fixture.open(std::slice::from_ref(&entry));

    let plans = plan_sync(&fixture.config, &project, false).unwrap();
    apply_sync(&mut project, &plans).unwrap();

    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].action, SyncAction::Ok);
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
    assert!(!fixture.root.join("skills.toml").exists());
    assert!(!fixture.root.join(".skills-manager").exists());
}

#[test]
fn a_changed_store_updates_the_copy_and_the_hashes() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::write(
        fixture.store.join("alpha").join("SKILL.md"),
        "name: alpha\nv2\n",
    )
    .unwrap();

    let plans = fixture.sync(std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Updated);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv2\n");
    let recorded = fixture.recorded("alpha");
    assert_eq!(recorded.written_hash, recorded.source_hash);
    assert_eq!(recorded.written_hash, fixture.source_hash());
    assert_eq!(
        recorded.installed_at, entry.installed_at,
        "an existing install keeps its date"
    );
}

#[test]
fn a_missing_copy_is_installed() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::remove_dir_all(fixture.dest()).unwrap();

    let plans = fixture.sync(std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Installed);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    assert_eq!(fixture.recorded("alpha").source_hash, fixture.source_hash());
}

#[test]
fn a_missing_pinned_copy_is_restored() {
    let fixture = Fixture::new();
    let entry = fixture.entry(true);
    fs::remove_dir_all(fixture.dest()).unwrap();

    let plans = fixture.sync(std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Restored);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    let recorded = fixture.recorded("alpha");
    assert!(recorded.pinned);
    assert_eq!(recorded.installed_at, entry.installed_at);
}

#[test]
fn a_pinned_copy_is_left_alone_even_without_a_store() {
    let fixture = Fixture::new();
    let entry = fixture.entry(true);
    fs::remove_dir_all(&fixture.store).unwrap();
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();

    let plans = fixture.plan(std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Pinned);
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
}

#[test]
fn a_locally_modified_copy_is_detached() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();

    let plans = fixture.sync(std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Detached);
    assert!(
        plans[0].source_dir.is_none(),
        "a detached copy must not carry a source to overwrite it with"
    );
    assert_eq!(fixture.read_dest("local.md"), "fork\n");
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");

    let project = fixture.reopen();
    let recorded = project.record.get("alpha", ".agents/skills").unwrap();
    assert!(recorded.detached);
    assert_eq!(
        recorded.content_hash,
        Some(hash::tree_hash(&fixture.dest()).unwrap())
    );
    assert!(recorded.detached_at.is_some_and(|at| at > 1_700_000_000));
    assert_eq!(recorded.written_hash, entry.written_hash);
    assert_eq!(recorded.installed_at, entry.installed_at);
}

#[test]
fn force_replaces_a_locally_modified_copy() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::write(fixture.dest().join("SKILL.md"), "hacked\n").unwrap();

    let plans = fixture.sync(std::slice::from_ref(&entry), true);

    assert_eq!(plans[0].action, SyncAction::Replaced);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    let project = fixture.reopen();
    let recorded = project.record.get("alpha", ".agents/skills").unwrap();
    assert!(!recorded.detached);
    assert_eq!(recorded.written_hash, fixture.source_hash());
}

#[test]
fn a_copy_without_state_that_matches_the_store_is_adopted() {
    let fixture = Fixture::new();
    fsops::copy_tree(&fixture.store.join("alpha"), &fixture.dest()).unwrap();

    let plans = fixture.sync(&[], false);

    assert_eq!(plans[0].action, SyncAction::Adopted);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    let recorded = fixture.recorded("alpha");
    assert!(!recorded.detached);
    assert_eq!(recorded.written_hash, fixture.source_hash());
}

#[test]
fn a_tombstone_is_ignored_and_never_adopted_again() {
    let fixture = Fixture::new();
    let tombstone = SkillState {
        content_hash: Some("blake3:local".to_owned()),
        detached_at: Some(1_760_600_000),
        detached: true,
        ..fixture.entry(false)
    };

    let fork = std::slice::from_ref(&tombstone);

    // Fork on disk without change: untouched and not reported as unmanaged
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();
    let plans = fixture.plan(fork, false);
    assert!(plans.is_empty());
}

#[test]
fn a_skill_the_store_no_longer_has_is_skipped() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::remove_dir_all(fixture.store.join("alpha")).unwrap();

    let plans = fixture.plan(std::slice::from_ref(&entry), false);
    assert_eq!(plans[0].action, SyncAction::Skipped);
    assert_eq!(plans[0].skip, Some(SkipReason::Unavailable));
    assert_eq!(plans[0].note(), Some("store unavailable"));
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
}

#[test]
fn without_a_store_every_entry_is_skipped_with_the_reason() {
    let mut fixture = Fixture::new();
    let entry = fixture.entry(false);
    fixture.config.store = None;

    let plans = fixture.plan(std::slice::from_ref(&entry), false);

    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].action, SyncAction::Skipped);
    assert_eq!(plans[0].skip, Some(SkipReason::NoStore));
    assert_eq!(plans[0].note(), Some("no store registered"));
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
}

#[test]
fn a_moved_skill_is_found_by_a_scan() {
    let fixture = Fixture::new();
    let entry = SkillState {
        source_rel: "old/alpha".to_owned(),
        ..fixture.entry(false)
    };

    let plans = fixture.plan(&[entry], false);

    assert_eq!(plans[0].action, SyncAction::Ok);
    assert_eq!(plans[0].source_rel, "alpha");
}

#[test]
fn unmanaged_directories_are_reported_but_never_touched() {
    let fixture = Fixture::new();
    let rogue = fixture.root.join(".agents/skills/rogue");
    fs::create_dir_all(&rogue).unwrap();
    fs::write(rogue.join("SKILL.md"), "hand made\n").unwrap();

    let plans = fixture.plan(&[], false);

    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].skill, "rogue");
    assert_eq!(plans[0].dest_rel, ".agents/skills/rogue");
    assert_eq!(plans[0].action, SyncAction::Unmanaged);
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
}

#[test]
fn the_target_recorded_in_the_state_is_used() {
    let fixture = Fixture::new();
    let entry = SkillState {
        target: "custom/skills".to_owned(),
        ..fixture.entry(false)
    };
    fs::create_dir_all(fixture.root.join("custom/skills")).unwrap();
    fs::rename(fixture.dest(), fixture.root.join("custom/skills/alpha")).unwrap();

    let plans = fixture.plan(&[entry], false);

    assert_eq!(plans[0].target, "custom/skills");
    assert_eq!(plans[0].dest_rel, "custom/skills/alpha");
    assert_eq!(plans[0].action, SyncAction::Ok);
}
