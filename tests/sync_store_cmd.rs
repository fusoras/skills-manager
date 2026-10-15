//! End-to-end tests of `skm sync` against the single global store:
//! where the skills come from, and which formats are refused.

mod common;

use std::path::{Path, PathBuf};

use common::{
    DEFAULT_CONFIG, Sandbox, failure_of, lines, manifest_of, read, state_of, stdout_of, tree, write,
};

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
fn sync_without_a_store_skips_and_writes_nothing() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    stdout_of(sandbox.skm().args(["store", "remove"]));
    let tree_before = tree(&project);
    let state_before = read(&state_of(&project));

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line(
                "skipped",
                "alpha",
                ".agents/skills/alpha",
                Some("no store registered")
            ),
            "1 entry: 1 skipped".to_owned(),
        ])
    );
    assert_eq!(
        tree(&project),
        tree_before,
        "a skipped sync writes nothing at all"
    );
    assert_eq!(read(&state_of(&project)), state_before);

    // The same is reported for a declaration that was never installed.
    write(&manifest_of(&project), "[[install]]\nskill = \"ghost\"\n");
    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));
    assert_eq!(
        output,
        lines(&[
            line(
                "skipped",
                "ghost",
                ".agents/skills/ghost",
                Some("no store registered")
            ),
            "1 entry: 1 skipped".to_owned(),
        ])
    );
}

#[test]
fn sync_follows_the_store_that_is_registered_now() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let other = sandbox.path("other-store");
    sandbox.skill(&other, "alpha", Some("alpha"));
    write(&other.join("alpha/v2.md"), "from the other store\n");
    sandbox.register_store(&other);

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("updated", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 updated".to_owned(),
        ])
    );
    assert!(
        dest(&project).join("v2.md").is_file(),
        "the copy follows the store that is registered now"
    );
}

#[test]
fn sync_uses_the_global_store_without_any_manifest_mapping() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    // The manifest declares the skill and nothing else: the store is global.
    write(&manifest_of(&project), "[[install]]\nskill = \"alpha\"\n");

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
fn sync_rejects_the_old_manifest_format() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();

    // Every old shape is rejected on its own, not only in combination.
    for old in [
        "[stores]\npersonal = \"/srv/store\"\n",
        "[[install]]\nskill = \"alpha\"\nstore = \"personal\"\n",
        "[stores]\npersonal = \"/srv/store\"\n\n\
         [[install]]\nskill = \"alpha\"\nstore = \"personal\"\n",
    ] {
        write(&manifest_of(&project), old);
        let (stdout, stderr) = failure_of(sandbox.skm_in(&project).arg("sync"));
        assert!(stdout.is_empty(), "{old}: {stdout}");
        assert!(stderr.contains("old manifest format"), "{old}: {stderr}");
        assert!(stderr.contains("store = \"...\""), "{old}: {stderr}");
    }
}

#[test]
fn sync_rejects_pack_entries() {
    let sandbox = Sandbox::new();
    let project = sandbox.project();
    write(
        &manifest_of(&project),
        "[[install]]\npack = \"web-stack\"\n",
    );

    let (stdout, stderr) = failure_of(sandbox.skm_in(&project).arg("sync"));

    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr
            .contains("error: packs are not supported yet (install entry `pack = \"web-stack\"`)"),
        "{stderr}"
    );
}
