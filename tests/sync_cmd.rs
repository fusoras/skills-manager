//! End-to-end tests of `skm sync`: reconciling a project with the global store.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{
    DEFAULT_CONFIG, Sandbox, lines, manifest_of, mtime, read, state_of, stdout_of, tree, value_of,
    write,
};

/// The body `Sandbox::skill` writes for a skill with frontmatter.
const SKILL_BODY: &str = "---\nname: alpha\ndescription: test skill\n---\n\n# Body\n";

/// The second version of the skill in the store.
const SKILL_V2: &str = "---\nname: alpha\ndescription: v2\n---\n\n# v2\n";

/// One line of the sync output: the keyword padded to the action column.
fn line(keyword: &str, skill: &str, dest: &str, note: Option<&str>) -> String {
    let mut text = format!("{keyword:<9} {skill} -> {dest}");
    if let Some(note) = note {
        text.push_str(&format!(" ({note})"));
    }
    text
}

/// A sandbox with a flat store holding `alpha` and a project that installed it.
fn installed() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.write_config(DEFAULT_CONFIG);
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    (sandbox, store)
}

/// The copy of `alpha` in the resolved target of the project.
fn dest(project: &Path) -> PathBuf {
    project.join(".agents").join("skills").join("alpha")
}

#[test]
fn sync_reports_ok_and_writes_nothing() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let manifest_before = read(&manifest_of(&project));
    let state_before = read(&state_of(&project));
    let mtime_before = mtime(&dest(&project).join("SKILL.md"));

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("ok", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 ok".to_owned(),
        ])
    );
    assert_eq!(read(&manifest_of(&project)), manifest_before);
    assert_eq!(read(&state_of(&project)), state_before);
    assert_eq!(mtime(&dest(&project).join("SKILL.md")), mtime_before);

    // Sync is idempotent: running it again reports the same.
    let second = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(second, output);
    assert_eq!(read(&state_of(&project)), state_before);
}

#[test]
fn sync_updates_a_copy_when_the_store_changes() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    let manifest_before = read(&manifest_of(&project));
    write(&store.join("alpha").join("SKILL.md"), SKILL_V2);

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("updated", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 updated".to_owned(),
        ])
    );
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_V2);
    assert_eq!(
        read(&manifest_of(&project)),
        manifest_before,
        "an update never touches the manifest"
    );
    let state = read(&state_of(&project));
    assert_eq!(
        value_of(&state, "written_hash"),
        value_of(&state, "source_hash"),
        "the new store content is recorded as both hashes"
    );

    let second = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(
        second,
        lines(&[
            line("ok", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 ok".to_owned(),
        ])
    );
}

#[test]
fn sync_installs_a_missing_copy() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    fs::remove_dir_all(dest(&project)).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("installed", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 installed".to_owned(),
        ])
    );
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_BODY);
}

#[test]
fn sync_restores_a_pinned_copy_that_was_deleted() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha", "--pin"]));
    fs::remove_dir_all(dest(&project)).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("restored", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 restored".to_owned(),
        ])
    );
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_BODY);
    assert_eq!(value_of(&read(&state_of(&project)), "pinned"), "true");
}

#[test]
fn sync_keeps_a_pinned_copy_untouched_when_the_store_changes() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha", "--pin"]));
    let before = tree(&dest(&project));
    write(&store.join("alpha").join("SKILL.md"), SKILL_V2);

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("pinned", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 pinned".to_owned(),
        ])
    );
    assert_eq!(tree(&dest(&project)), before);
    assert_eq!(
        value_of(&read(&state_of(&project)), "source_hash"),
        value_of(&read(&state_of(&project)), "written_hash"),
        "the recorded hashes still describe the installed copy"
    );
}

#[test]
fn sync_detaches_a_locally_modified_copy() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let dest = dest(&project);
    write(&dest.join("local.md"), "my fork\n");
    let tree_before = tree(&dest);

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line(
                "detached",
                "alpha",
                ".agents/skills/alpha",
                Some("local modifications")
            ),
            "1 entry: 1 detached".to_owned(),
        ])
    );
    // The files are exactly the ones the user left behind.
    assert_eq!(tree(&dest), tree_before);
    assert_eq!(read(&dest.join("local.md")), "my fork\n");
    assert_eq!(read(&dest.join("SKILL.md")), SKILL_BODY);
    // The declaration is gone; the manifest keeps everything else.
    let manifest = read(&manifest_of(&project));
    assert!(!manifest.contains("[[install]]"), "{manifest}");
    // The tombstone records the fork.
    let state = read(&state_of(&project));
    assert_eq!(value_of(&state, "detached"), "true");
    assert!(value_of(&state, "content_hash").starts_with("blake3:"));
    assert!(
        value_of(&state, "detached_at").parse::<i64>().unwrap() > 1_700_000_000,
        "{state}"
    );
}

#[test]
fn sync_ignores_the_tombstone_on_the_next_run() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let dest = dest(&project);
    write(&dest.join("local.md"), "my fork\n");
    stdout_of(sandbox.skm_in(&project).arg("sync"));
    let state_before = read(&state_of(&project));
    let manifest_before = read(&manifest_of(&project));

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        "Nothing to sync: no skills declared in skills.toml and nothing found in the target directories.\n"
    );
    assert_eq!(read(&state_of(&project)), state_before);
    assert_eq!(read(&manifest_of(&project)), manifest_before);
    assert_eq!(read(&dest.join("local.md")), "my fork\n");
}

#[test]
fn sync_replaces_a_modified_copy_with_force() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let dest = dest(&project);
    write(&dest.join("SKILL.md"), "hacked\n");

    let output = stdout_of(sandbox.skm_in(&project).args(["sync", "--force"]));

    assert_eq!(
        output,
        lines(&[
            line("replaced", "alpha", ".agents/skills/alpha", Some("forced")),
            "1 entry: 1 replaced".to_owned(),
        ])
    );
    assert_eq!(read(&dest.join("SKILL.md")), SKILL_BODY);
    assert!(
        read(&manifest_of(&project)).contains("[[install]]"),
        "a forced replacement keeps the declaration"
    );
    let state = read(&state_of(&project));
    assert_eq!(
        value_of(&state, "written_hash"),
        value_of(&state, "source_hash")
    );
}

#[test]
fn sync_adopts_a_copy_that_matches_the_store() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    // A fresh clone: the files are there, the local state is not.
    fs::remove_dir_all(project.join(".skills-manager")).unwrap();
    let mtime_before = mtime(&dest(&project).join("SKILL.md"));

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("adopted", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 adopted".to_owned(),
        ])
    );
    assert_eq!(mtime(&dest(&project).join("SKILL.md")), mtime_before);
    let state = read(&state_of(&project));
    assert_eq!(
        value_of(&state, "written_hash"),
        value_of(&state, "source_hash")
    );
}

#[test]
fn sync_detaches_a_copy_without_state_that_differs_from_the_store() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let dest = dest(&project);
    fs::remove_dir_all(project.join(".skills-manager")).unwrap();
    write(&dest.join("local.md"), "my fork\n");

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line(
                "detached",
                "alpha",
                ".agents/skills/alpha",
                Some("local modifications")
            ),
            "1 entry: 1 detached".to_owned(),
        ])
    );
    assert!(!read(&manifest_of(&project)).contains("[[install]]"));
    let state = read(&state_of(&project));
    assert_eq!(value_of(&state, "detached"), "true");
    // Nothing was ever written by skm: both hashes describe the fork.
    assert_eq!(
        value_of(&state, "content_hash"),
        value_of(&state, "written_hash")
    );
    assert_eq!(read(&dest.join("local.md")), "my fork\n");
}

#[test]
fn sync_reports_unmanaged_directories_and_leaves_them_alone() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let rogue = project.join(".agents/skills/rogue");
    write(&rogue.join("SKILL.md"), "hand made\n");

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("ok", "alpha", ".agents/skills/alpha", None),
            line(
                "unmanaged",
                "rogue",
                ".agents/skills/rogue",
                Some("not managed by skm")
            ),
            "2 entries: 1 ok, 1 unmanaged".to_owned(),
        ])
    );
    assert_eq!(read(&rogue.join("SKILL.md")), "hand made\n");
}

#[test]
fn sync_skips_a_skill_the_store_no_longer_has() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    fs::remove_dir_all(store.join("alpha")).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line(
                "skipped",
                "alpha",
                ".agents/skills/alpha",
                Some("store unavailable")
            ),
            "1 entry: 1 skipped".to_owned(),
        ])
    );
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_BODY);
}

#[test]
fn sync_uses_the_target_recorded_in_the_state() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));
    stdout_of(
        sandbox
            .skm_in(&project)
            .args(["add", "alpha", "--dir", "custom/skills"]),
    );

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("ok", "alpha", "custom/skills/alpha", None),
            "1 entry: 1 ok".to_owned(),
        ])
    );

    // Updates land in that target too.
    write(&store.join("alpha").join("SKILL.md"), SKILL_V2);
    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(
        output,
        lines(&[
            line("updated", "alpha", "custom/skills/alpha", None),
            "1 entry: 1 updated".to_owned(),
        ])
    );
    assert_eq!(
        read(&project.join("custom/skills/alpha/SKILL.md")),
        SKILL_V2
    );
}

#[test]
fn sync_dry_run_plans_everything_and_writes_nothing() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    sandbox.skill(&store, "beta", None);
    stdout_of(sandbox.skm_in(&project).args(["add", "beta"]));
    write(&store.join("alpha").join("SKILL.md"), SKILL_V2);
    write(&project.join(".agents/skills/beta/local.md"), "my fork\n");
    let manifest_before = read(&manifest_of(&project));
    let state_before = read(&state_of(&project));
    let tree_before = tree(&project);
    let mtime_before = mtime(&dest(&project).join("SKILL.md"));

    let output = stdout_of(sandbox.skm_in(&project).args(["sync", "--dry-run"]));

    assert_eq!(
        output,
        lines(&[
            line("updated", "alpha", ".agents/skills/alpha", None),
            line(
                "detached",
                "beta",
                ".agents/skills/beta",
                Some("local modifications")
            ),
            "2 entries: 1 updated, 1 detached".to_owned(),
            "Dry run: nothing was written.".to_owned(),
        ])
    );
    assert_eq!(read(&manifest_of(&project)), manifest_before);
    assert_eq!(read(&state_of(&project)), state_before);
    assert_eq!(tree(&project), tree_before);
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_BODY);
    assert_eq!(mtime(&dest(&project).join("SKILL.md")), mtime_before);

    // The real run does exactly what the plan announced.
    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(
        output,
        lines(&[
            line("updated", "alpha", ".agents/skills/alpha", None),
            line(
                "detached",
                "beta",
                ".agents/skills/beta",
                Some("local modifications")
            ),
            "2 entries: 1 updated, 1 detached".to_owned(),
        ])
    );
    assert_eq!(read(&dest(&project).join("SKILL.md")), SKILL_V2);
    let manifest = read(&manifest_of(&project));
    assert!(manifest.contains("skill = \"alpha\""), "{manifest}");
    assert!(!manifest.contains("skill = \"beta\""), "{manifest}");
}

#[test]
fn sync_preserves_the_manifest_comments_when_it_detaches() {
    let (sandbox, store) = installed();
    let project = sandbox.project();
    sandbox.skill(&store, "beta", None);
    stdout_of(sandbox.skm_in(&project).args(["add", "beta"]));
    write(
        &manifest_of(&project),
        "# team skills\n\n\
         [[install]]\n# keep this one\nskill = \"alpha\"\n\n\
         [[install]]\nskill = \"beta\"\n",
    );
    write(&project.join(".agents/skills/beta/local.md"), "my fork\n");

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert!(output.contains("detached  beta"), "{output}");
    let after = read(&manifest_of(&project));
    assert!(after.starts_with("# team skills\n"), "{after}");
    assert!(after.contains("# keep this one"), "{after}");
    assert!(after.contains("skill = \"alpha\""), "{after}");
    assert!(!after.contains("skill = \"beta\""), "{after}");

    // The rewritten manifest still parses: a later sync reads it back.
    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(
        output,
        lines(&[
            line("ok", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 ok".to_owned(),
        ])
    );
}

#[test]
fn sync_reports_an_empty_project() {
    let sandbox = Sandbox::new();
    let project = sandbox.project();

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        "Nothing to sync: no skills declared in skills.toml and nothing found in the target directories.\n"
    );
    assert_eq!(tree(&project), Vec::<String>::new());
}
