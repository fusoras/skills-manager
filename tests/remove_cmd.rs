//! End-to-end tests of `skm remove`: deleting copies and declarations.

mod common;

use std::path::PathBuf;

use common::{Sandbox, failure_of, manifest_of, read, state_of, stdout_of, tree, write};

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
fn remove_deletes_the_copy_the_declaration_and_the_state() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!project.join(".agents/skills/alpha").exists());
    let manifest = read(&manifest_of(&project));
    assert!(!manifest.contains("[[install]]"), "{manifest}");
    assert!(!read(&state_of(&project)).contains("[[skill]]"));
}

#[test]
fn remove_refuses_local_modifications_unless_forced() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let local = project.join(".agents/skills/alpha/notes.md");
    write(&local, "local work\n");
    let before_state = read(&state_of(&project));

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
    assert_eq!(read(&state_of(&project)), before_state);
    assert!(read(&manifest_of(&project)).contains("[[install]]"));

    let output = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "alpha", "--force"]),
    );
    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!project.join(".agents/skills/alpha").exists());
}

#[test]
fn remove_of_a_never_installed_declaration_only_drops_it() {
    let (sandbox, _store, project) = ready();
    write(&manifest_of(&project), "[[install]]\nskill = \"beta\"\n");

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "beta"]));

    assert_eq!(output, "Un-declared beta (was never installed)\n");
    let manifest = read(&manifest_of(&project));
    assert!(!manifest.contains("[[install]]"), "{manifest}");
    assert!(
        !state_of(&project).exists(),
        "a project with nothing installed must not gain a state file"
    );
    assert!(!project.join(".agents").exists());
}

#[test]
fn remove_rejects_skills_that_are_not_declared() {
    let (sandbox, _store, project) = ready();

    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "ghost"]));
    assert!(
        stderr.contains("error: skill 'ghost' is not declared in skills.toml"),
        "{stderr}"
    );

    // Installed by skm, but the declaration was dropped by hand.
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    write(&manifest_of(&project), "");
    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "alpha"]));
    assert!(
        stderr.contains(
            "error: skill 'alpha' is not declared in skills.toml but skm installed it in \
             .agents/skills/alpha; delete that directory by hand"
        ),
        "{stderr}"
    );
    assert!(project.join(".agents/skills/alpha").exists());
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
}

#[test]
fn remove_needs_force_for_a_directory_skm_did_not_write() {
    let (sandbox, _store, project) = ready();
    write(&manifest_of(&project), "[[install]]\nskill = \"alpha\"\n");
    write(
        &project.join(".agents/skills/alpha/handwritten.md"),
        "mine\n",
    );

    let (_, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "alpha"]));
    assert!(
        stderr.contains(
            "error: .agents/skills/alpha exists but is not managed by skm; \
             pass --force to delete it"
        ),
        "{stderr}"
    );
    assert!(project.join(".agents/skills/alpha/handwritten.md").exists());

    let output = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "alpha", "--force"]),
    );
    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!project.join(".agents/skills/alpha").exists());
}

#[test]
fn remove_dry_run_writes_nothing() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    let before = tree(&project);
    let before_manifest = read(&manifest_of(&project));

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
    assert_eq!(read(&manifest_of(&project)), before_manifest);
}

#[test]
fn remove_takes_several_skills_and_keeps_the_comments() {
    let (sandbox, _store, project) = ready();
    write(
        &manifest_of(&project),
        "# my project\n\n[defaults]\ntarget = \".agents/skills\"\n",
    );
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha", "beta"]));

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha", "beta"]));

    assert_eq!(
        output,
        "Removed alpha (.agents/skills/alpha)\nRemoved beta (.agents/skills/beta)\n"
    );
    assert!(!project.join(".agents/skills/alpha").exists());
    assert!(!project.join(".agents/skills/beta").exists());
    let manifest = read(&manifest_of(&project));
    assert!(manifest.starts_with("# my project\n"), "{manifest}");
    assert!(!manifest.contains("[[install]]"), "{manifest}");
    assert!(!read(&state_of(&project)).contains("[[skill]]"));
}

#[test]
fn remove_works_when_the_copy_was_already_deleted_by_hand() {
    let (sandbox, _store, project) = ready();
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));
    std::fs::remove_dir_all(project.join(".agents/skills/alpha")).unwrap();

    let output = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));

    assert_eq!(output, "Removed alpha (.agents/skills/alpha)\n");
    assert!(!read(&state_of(&project)).contains("[[skill]]"));
    assert!(!read(&manifest_of(&project)).contains("[[install]]"));
}
