//! End-to-end tests of `skm remove`: deleting copies and unregistering.
//! Projects remain 100% clean: no `skills.toml` and no `.skills-manager/`.

mod common;

use std::path::PathBuf;

use common::{Sandbox, assert_clean_project, failure_of, read, stdout_of, tree, write};

/// Store with `alpha` and `beta`, registered globally, plus a project.
fn ready() -> (Sandbox, PathBuf, PathBuf) {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.skill(&store, "beta", Some("beta"));
    sandbox.register_store(&store);
    let project = sandbox.project();
    (sandbox, store, project)
}

#[test]
fn remove_deletes_the_copy_and_updates_central_registry() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!project.join(".agents/skills/alpha").exists());
    assert_clean_project(&project);

    let registry = sandbox.registry_text();
    assert!(!registry.contains("name = \"alpha\""), "{registry}");
}

#[test]
fn remove_refuses_local_modifications_unless_forced() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let local = project.join(".agents/skills/alpha/notes.md");
    write(&local, "local work\n");
    let before_registry = sandbox.registry_text();

    let (stdout, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(stdout, "");
    assert!(
        stderr.contains(
            "error: skill 'alpha' was modified in .agents/skills/alpha; \
             pass --force to delete it anyway"
        ),
        "{stderr}"
    );
    assert_eq!(read(&local), "local work\n", "nothing may be deleted");
    assert_eq!(sandbox.registry_text(), before_registry);
    assert_clean_project(&project);

    let output = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "alpha", "--force"]),
    );
    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!project.join(".agents/skills/alpha").exists());
    assert_clean_project(&project);
}

#[test]
fn remove_rejects_skills_that_are_not_registered() {
    let (sandbox, _store, project) = ready();

    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "ghost"]));
    assert!(
        stderr.contains("error: skill 'ghost' is not registered in this project"),
        "{stderr}"
    );
    assert_clean_project(&project);
}

#[test]
fn remove_requires_a_registered_store() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    stdout_of(sandbox.skm().args(["store", "remove"]));
    let before = tree(&project);

    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(
        stderr,
        "error: no store registered; run `skm store add <path>`\n"
    );
    assert_eq!(tree(&project), before, "nothing may be deleted");
    assert_clean_project(&project);
}

#[test]
fn remove_dry_run_writes_nothing() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let before = tree(&project);
    let before_registry = sandbox.registry_text();

    let output = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "alpha", "--dry-run"]),
    );

    assert_eq!(
        output,
        "Would remove alpha (.agents/skills/alpha)\nDry run: nothing was written.\n"
    );
    assert_eq!(tree(&project), before);
    assert_eq!(sandbox.registry_text(), before_registry);
    assert_clean_project(&project);
}

#[test]
fn remove_takes_several_skills() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha", "beta"]));

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha", "beta"]));

    assert_eq!(
        output,
        "Removed alpha (.agents/skills/alpha)\nRemoved beta (.agents/skills/beta)\n"
    );
    assert!(!project.join(".agents/skills/alpha").exists());
    assert!(!project.join(".agents/skills/beta").exists());
    assert_clean_project(&project);

    let registry = sandbox.registry_text();
    assert!(!registry.contains("name = \"alpha\""), "{registry}");
    assert!(!registry.contains("name = \"beta\""), "{registry}");
}

#[test]
fn remove_works_when_the_copy_was_already_deleted_by_hand() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    std::fs::remove_dir_all(project.join(".agents/skills/alpha")).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert_clean_project(&project);
    assert!(!sandbox.registry_text().contains("name = \"alpha\""));
}
