//! End-to-end tests of `skm status`: the health of every registered skill.
//! Projects remain 100% clean: no `skills.toml` and no `.skills-manager/`.

mod common;

use std::fs;

use common::{Sandbox, assert_clean_project, rows, stdout_of, table_cells, tree, write};

#[test]
fn status_reports_every_state() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    let names = ["alpha", "beta", "delta", "gamma"];
    for name in names {
        sandbox.skill(&store, name, Some(name));
    }
    sandbox.register_store(&store);
    let project = sandbox.project();

    // alpha stays untouched, beta gets a local edit, gamma a store update,
    // delta is pinned.
    stdout_of(
        sandbox
            .skm_in(&project)
            .args(["add", "alpha", "beta", "gamma"]),
    );
    stdout_of(sandbox.skm_in(&project).args(["add", "delta", "--pin"]));
    write(
        &project.join(".agents/skills/beta/local.md"),
        "local work\n",
    );
    write(&store.join("gamma/SKILL.md"), "---\nname: gamma\nv2\n---\n");
    write(
        &project.join(".agents/skills/rogue/SKILL.md"),
        "hand made\n",
    );
    fs::create_dir_all(project.join(".agents/skills/.hidden")).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "ok"],
            &["beta", ".agents/skills", "modified"],
            &["delta", ".agents/skills", "pinned"],
            &["gamma", ".agents/skills", "behind"],
            &["rogue", ".agents/skills", "unmanaged"],
        ])
    );
    assert_clean_project(&project);
}

#[test]
fn status_without_a_store_reports_skipped() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    stdout_of(sandbox.skm().args(["store", "remove"]));
    let before_tree = tree(&project);
    let before_registry = sandbox.registry_text();

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "skipped (no store registered)"],
        ])
    );
    assert_eq!(tree(&project), before_tree, "status must not write");
    assert_eq!(sandbox.registry_text(), before_registry);
    assert_clean_project(&project);
}

#[test]
fn status_writes_nothing() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    // Both sides change, so a status that wrote hashes would hide the drift.
    write(
        &project.join(".agents/skills/alpha/local.md"),
        "local work\n",
    );
    write(&store.join("alpha/SKILL.md"), "---\nname: alpha\nv2\n---\n");
    let before_tree = tree(&project);
    let before_registry = sandbox.registry_text();

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert!(output.contains("modified"), "{output}");
    assert_eq!(tree(&project), before_tree);
    assert_eq!(
        sandbox.registry_text(),
        before_registry,
        "status must not refresh the recorded hashes"
    );
    assert_clean_project(&project);
}

#[test]
fn status_uses_the_target_recorded_in_the_state() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(
        sandbox
            .skm_in(&project)
            .args(["add", "alpha", "--dir", "custom/skills"]),
    );

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", "custom/skills", "ok"],
        ])
    );
    assert_clean_project(&project);
}

#[test]
fn status_reports_an_empty_project() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        output,
        "Nothing to report: no skills registered in this project and nothing found in the target directories.\n"
    );
    assert_eq!(tree(&project), Vec::<String>::new());
    assert_clean_project(&project);
}

#[test]
fn status_finds_the_project_root_from_a_subdirectory() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.skill(&store, "beta", Some("beta"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let nested = project.join("src").join("deep");
    fs::create_dir_all(&nested).unwrap();

    let output = stdout_of(sandbox.skm_in(&nested).args(["status"]));

    assert!(output.contains("ok"), "{output}");

    // `add` from a subdirectory writes to the same project.
    let output = stdout_of(sandbox.skm_in(&nested).args(["add", "beta"]));
    assert_eq!(output, "Installed beta -> .agents/skills/beta\n");
    assert!(project.join(".agents/skills/beta/SKILL.md").is_file());
    assert_clean_project(&nested);
    assert_clean_project(&project);
}

#[test]
fn status_reports_a_detached_fork() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    write(
        &project.join(".agents/skills/alpha/local.md"),
        "local work\n",
    );
    stdout_of(sandbox.skm_in(&project).args(["sync"]));

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "detached"],
        ])
    );
    assert_clean_project(&project);
}

#[test]
fn status_keeps_the_store_untouched() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let before = tree(&store);

    stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(tree(&store), before);
    assert_clean_project(&project);
}
