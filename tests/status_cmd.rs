//! End-to-end tests of `skm status`: the health of every declared skill.

mod common;

use std::fs;

use common::{
    Sandbox, failure_of, manifest_of, read, rows, state_of, stdout_of, table_cells, tree, write,
};

/// Manifest declaring every skill of the store.
fn manifest_for(skills: &[&str]) -> String {
    let mut text = String::new();
    for skill in skills {
        text.push_str(&format!("[[install]]\nskill = \"{skill}\"\n\n"));
    }
    text
}

#[test]
fn status_reports_every_state() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    let names = ["alpha", "beta", "delta", "epsilon", "gamma"];
    for name in names {
        sandbox.skill(&store, name, Some(name));
    }
    sandbox.register_store(&store);
    let project = sandbox.project();
    write(&manifest_of(&project), &manifest_for(&names));

    // alpha stays untouched, beta gets a local edit, gamma a store update,
    // delta is pinned and epsilon is never installed.
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
            &["epsilon", ".agents/skills", "missing"],
            &["gamma", ".agents/skills", "behind"],
            &["rogue", ".agents/skills", "unmanaged"],
        ])
    );
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
    let before_state = read(&state_of(&project));

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "skipped (no store registered)"],
        ])
    );
    assert_eq!(tree(&project), before_tree, "status must not write");
    assert_eq!(read(&state_of(&project)), before_state);
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
    let before_manifest = read(&manifest_of(&project));
    let before_state = read(&state_of(&project));

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert!(output.contains("modified"), "{output}");
    assert_eq!(tree(&project), before_tree);
    assert_eq!(read(&manifest_of(&project)), before_manifest);
    assert_eq!(
        read(&state_of(&project)),
        before_state,
        "status must not refresh the recorded hashes"
    );
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
}

#[test]
fn status_reports_an_empty_project_and_state_only_copies() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();

    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(
        output,
        "Nothing to report: no skills declared in skills.toml and nothing found in the target directories.\n"
    );
    assert_eq!(tree(&project), Vec::<String>::new());

    // A copy installed by skm whose declaration was dropped by hand is still
    // reported, so the state file is never a blind spot.
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    write(&manifest_of(&project), "");
    let output = stdout_of(sandbox.skm_in(&project).args(["status"]));
    assert_eq!(
        table_cells(&output),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "ok"],
        ])
    );
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
    assert!(!nested.join("skills.toml").exists());
    assert!(manifest_of(&project).is_file());
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
}

#[test]
fn status_rejects_pack_entries() {
    let sandbox = Sandbox::new();
    let project = sandbox.project();
    write(
        &manifest_of(&project),
        "[[install]]\npack = \"web-stack\"\n",
    );

    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["status"]));

    assert!(
        stderr.contains("packs are not supported yet (install entry `pack = \"web-stack\"`)"),
        "{stderr}"
    );
}

#[test]
fn status_keeps_the_store_untouched() {
    // The store is read only for `status`, exactly like for `add`.
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let before = tree(&store);

    stdout_of(sandbox.skm_in(&project).args(["status"]));

    assert_eq!(tree(&store), before);
}
