//! End-to-end tests of `skm sync` against the single global store:
//! where the skills come from, and that target projects remain 100% clean.

mod common;

use std::path::{Path, PathBuf};

use common::{DEFAULT_CONFIG, Sandbox, assert_clean_project, lines, read, stdout_of, tree};

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
    let registry_before = sandbox.registry_text();

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
    assert_eq!(sandbox.registry_text(), registry_before);
    assert_clean_project(&project);
}

#[test]
fn sync_follows_the_store_that_is_registered_now() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();
    let other = sandbox.path("other-store");
    sandbox.skill(&other, "alpha", Some("alpha"));
    common::write(&other.join("alpha/v2.md"), "from the other store\n");
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
    assert_clean_project(&project);
}

#[test]
fn sync_never_creates_metadata_files_in_project() {
    let (sandbox, _store) = installed();
    let project = sandbox.project();

    common::write(
        &project.join("Cargo.toml"),
        "[package]\nname = \"my-project\"\n",
    );
    common::write(&project.join("README.md"), "# My Clean Project\n");

    let output = stdout_of(sandbox.skm_in(&project).arg("sync"));

    assert_eq!(
        output,
        lines(&[
            line("ok", "alpha", ".agents/skills/alpha", None),
            "1 entry: 1 ok".to_owned(),
        ])
    );
    assert_clean_project(&project);
    assert_eq!(read(&project.join("README.md")), "# My Clean Project\n");
}
