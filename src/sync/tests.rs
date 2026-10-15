//! Unit tests of the sync planner and applier.

use super::*;
use crate::project;
use crate::state::State;
use std::fs;
use tempfile::TempDir;

/// A store with one skill, a project, and the config that binds them.
struct Fixture {
    _tmp: TempDir,
    store: PathBuf,
    root: PathBuf,
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

        let config = Config {
            store: Some(store.display().to_string()),
            default_target: ".agents/skills".to_owned(),
            ..Config::default()
        };

        Self {
            _tmp: tmp,
            store,
            root,
            config,
        }
    }

    /// Manifest declaring `alpha` in the global store.
    fn manifest(&self) -> String {
        "[[install]]\nskill = \"alpha\"\n".to_owned()
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

    /// Write the manifest and the state, then open the project.
    fn open(&self, manifest: &str, entries: &[SkillState]) -> Project {
        fs::write(project::manifest_path(&self.root), manifest).unwrap();
        let mut state = State::default();
        for entry in entries {
            state.upsert(entry.clone());
        }
        state::save(&project::state_path(&self.root), &state).unwrap();
        Project::open_at(&self.root).unwrap()
    }

    fn plan(&self, manifest: &str, entries: &[SkillState], force: bool) -> Vec<SyncPlan> {
        let project = self.open(manifest, entries);
        plan_sync(&self.config, &project, force).unwrap()
    }

    /// Plan and apply, as the command does, and return the plans.
    fn sync(&self, manifest: &str, entries: &[SkillState], force: bool) -> Vec<SyncPlan> {
        let mut project = self.open(manifest, entries);
        let plans = plan_sync(&self.config, &project, force).unwrap();
        apply_sync(&mut project, &plans).unwrap();
        plans
    }

    /// Read the project back after a sync.
    fn reopen(&self) -> Project {
        Project::open_at(&self.root).unwrap()
    }

    fn recorded(&self, name: &str) -> SkillState {
        self.reopen()
            .state
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
    let mut project = fixture.open(&fixture.manifest(), std::slice::from_ref(&entry));
    let manifest_before = fs::read_to_string(project.manifest_path()).unwrap();
    let state_before = fs::read_to_string(project.state_path()).unwrap();

    let plans = plan_sync(&fixture.config, &project, false).unwrap();
    apply_sync(&mut project, &plans).unwrap();

    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].action, SyncAction::Ok);
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
    assert_eq!(
        fs::read_to_string(project.manifest_path()).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read_to_string(project.state_path()).unwrap(),
        state_before,
        "an unchanged copy must not be re-dated"
    );
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

    let plans = fixture.sync(&fixture.manifest(), std::slice::from_ref(&entry), false);

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

    let plans = fixture.sync(&fixture.manifest(), std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Installed);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    assert_eq!(fixture.recorded("alpha").source_hash, fixture.source_hash());
}

#[test]
fn a_missing_pinned_copy_is_restored() {
    let fixture = Fixture::new();
    let entry = fixture.entry(true);
    fs::remove_dir_all(fixture.dest()).unwrap();

    let plans = fixture.sync(&fixture.manifest(), std::slice::from_ref(&entry), false);

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

    let plans = fixture.plan(&fixture.manifest(), std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Pinned);
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());
}

#[test]
fn a_locally_modified_copy_is_detached() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();

    let plans = fixture.sync(&fixture.manifest(), std::slice::from_ref(&entry), false);

    assert_eq!(plans[0].action, SyncAction::Detached);
    assert!(
        plans[0].source_dir.is_none(),
        "a detached copy must not carry a source to overwrite it with"
    );
    // The files stay exactly as the user left them.
    assert_eq!(fixture.read_dest("local.md"), "fork\n");
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    // The declaration is gone and a tombstone took its place.
    let project = fixture.reopen();
    assert!(project.manifest.install.is_empty());
    let recorded = project.state.get("alpha", ".agents/skills").unwrap();
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

    let plans = fixture.sync(&fixture.manifest(), std::slice::from_ref(&entry), true);

    assert_eq!(plans[0].action, SyncAction::Replaced);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    let project = fixture.reopen();
    assert_eq!(project.manifest.install.len(), 1, "the declaration stays");
    let recorded = project.state.get("alpha", ".agents/skills").unwrap();
    assert!(!recorded.detached);
    assert_eq!(recorded.written_hash, fixture.source_hash());
}

#[test]
fn a_copy_without_state_that_matches_the_store_is_adopted() {
    let fixture = Fixture::new();
    fsops::copy_tree(&fixture.store.join("alpha"), &fixture.dest()).unwrap();

    let plans = fixture.sync(&fixture.manifest(), &[], false);

    assert_eq!(plans[0].action, SyncAction::Adopted);
    assert_eq!(fixture.read_dest("SKILL.md"), "name: alpha\nv1\n");
    let recorded = fixture.recorded("alpha");
    assert!(!recorded.detached);
    assert_eq!(recorded.written_hash, fixture.source_hash());
}

#[test]
fn a_copy_without_state_that_differs_is_detached() {
    let fixture = Fixture::new();
    fsops::copy_tree(&fixture.store.join("alpha"), &fixture.dest()).unwrap();
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();

    let plans = fixture.sync(&fixture.manifest(), &[], false);

    assert_eq!(plans[0].action, SyncAction::Detached);
    let project = fixture.reopen();
    assert!(project.manifest.install.is_empty());
    let recorded = project.state.get("alpha", ".agents/skills").unwrap();
    assert!(recorded.detached);
    // skm never wrote this copy: both hashes describe the moment.
    assert_eq!(
        recorded.content_hash,
        Some(hash::tree_hash(&fixture.dest()).unwrap())
    );
    assert_eq!(
        recorded.written_hash,
        recorded.content_hash.clone().unwrap()
    );
    assert_eq!(recorded.source_hash, fixture.source_hash());
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

    // No declaration: the fork is not reported at all.
    assert!(fixture.plan("", fork, false).is_empty());

    // Declared again by hand: still a fork, so it detaches instead of adopting.
    fs::write(fixture.dest().join("local.md"), "fork\n").unwrap();
    let plans = fixture.plan(&fixture.manifest(), fork, false);
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].action, SyncAction::Detached);

    // Restored to the pristine store content, the copy is adopted: a tombstone
    // is ignored, so this is just a copy that happens to match the store.
    fs::remove_file(fixture.dest().join("local.md")).unwrap();
    let plans = fixture.plan(&fixture.manifest(), fork, false);
    assert_eq!(plans[0].action, SyncAction::Adopted);
}

#[test]
fn a_skill_the_store_no_longer_has_is_skipped() {
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::remove_dir_all(fixture.store.join("alpha")).unwrap();

    let plans = fixture.plan(&fixture.manifest(), std::slice::from_ref(&entry), false);
    assert_eq!(plans[0].action, SyncAction::Skipped);
    assert_eq!(plans[0].skip, Some(SkipReason::Unavailable));
    assert_eq!(plans[0].note(), Some("store unavailable"));
    assert!(plans[0].entry.is_none() && plans[0].source_dir.is_none());

    // A store directory that disappeared is just as unavailable.
    let fixture = Fixture::new();
    let entry = fixture.entry(false);
    fs::remove_dir_all(&fixture.store).unwrap();
    let plans = fixture.plan(&fixture.manifest(), std::slice::from_ref(&entry), false);
    assert_eq!(plans[0].skip, Some(SkipReason::Unavailable));
}

#[test]
fn without_a_store_every_entry_is_skipped_with_the_reason() {
    let mut fixture = Fixture::new();
    let entry = fixture.entry(false);
    fixture.config.store = None;

    // A declared skill with nothing installed, and one with a recorded copy.
    let plans = fixture.plan(&fixture.manifest(), std::slice::from_ref(&entry), false);

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

    let plans = fixture.plan(&fixture.manifest(), &[entry], false);

    assert_eq!(plans[0].action, SyncAction::Ok);
    assert_eq!(plans[0].source_rel, "alpha");
}

#[test]
fn unmanaged_directories_are_reported_but_never_touched() {
    let fixture = Fixture::new();
    let rogue = fixture.root.join(".agents/skills/rogue");
    fs::create_dir_all(&rogue).unwrap();
    fs::write(rogue.join("SKILL.md"), "hand made\n").unwrap();

    let plans = fixture.plan("", &[], false);

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

    let plans = fixture.plan(&fixture.manifest(), &[entry], false);

    assert_eq!(plans[0].target, "custom/skills");
    assert_eq!(plans[0].dest_rel, "custom/skills/alpha");
    assert_eq!(plans[0].action, SyncAction::Ok);
}
