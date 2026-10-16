//! End-to-end tests for skill packs (`skm pack` and `--pack` flags).
//!
//! Packs are named collections of skills defined in `<store>/packs/<name>.toml`.
//! They live in the store only: target projects remain 100% clean (zero metadata files).

mod common;

use std::fs;
use std::path::PathBuf;

use common::{Sandbox, assert_clean_project, failure_of, rows, stdout_of, table_cells, write};

/// Create a store with skills and packs.
fn setup_store(sandbox: &Sandbox) -> PathBuf {
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    write(&store.join("alpha/run.sh"), "#!/bin/sh\necho alpha\n");

    sandbox.skill(&store, "beta", Some("beta"));
    sandbox.skill(&store, "gamma", Some("gamma"));

    // Packs directory in store
    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).unwrap();

    write(
        &packs_dir.join("frontend.toml"),
        r#"
name = "frontend"
description = "Frontend tools"
skills = ["alpha", "beta"]
"#,
    );

    write(
        &packs_dir.join("fullstack.toml"),
        r#"
description = "Full stack tools"
skills = ["beta", "gamma"]
"#,
    );

    store
}

fn ready() -> (Sandbox, PathBuf, PathBuf) {
    let sandbox = Sandbox::new();
    let store = setup_store(&sandbox);
    sandbox.register_store(&store);
    let project = sandbox.project();
    (sandbox, store, project)
}

#[test]
fn pack_list_and_show() {
    let (sandbox, _store, project) = ready();

    // Plain `skm pack` or `skm pack list`
    let output = stdout_of(sandbox.skm_in(&project).args(["pack"]));
    assert_eq!(
        table_cells(&output),
        rows(&[
            &["PACK", "SKILLS", "DESCRIPTION"],
            &["frontend", "alpha, beta", "Frontend tools"],
            &["fullstack", "beta, gamma", "Full stack tools"],
        ])
    );

    // `skm list --packs` produces the exact same output
    let output_list = stdout_of(sandbox.skm_in(&project).args(["list", "--packs"]));
    assert_eq!(output, output_list);

    // `skm pack show frontend`
    let output_show = stdout_of(sandbox.skm_in(&project).args(["pack", "show", "frontend"]));
    assert!(output_show.contains("name         frontend"));
    assert!(output_show.contains("description  Frontend tools"));
    assert!(output_show.contains("skills       2"));
    assert!(output_show.contains("- alpha"));
    assert!(output_show.contains("- beta"));

    // Case-insensitive show
    let output_case = stdout_of(sandbox.skm_in(&project).args(["pack", "show", "Frontend"]));
    assert_eq!(output_show, output_case);
}

#[test]
fn pack_add_installs_all_member_skills_and_keeps_project_clean() {
    let (sandbox, _store, project) = ready();

    // Install pack using `skm add --pack frontend`
    let output = stdout_of(sandbox.skm_in(&project).args(["add", "--pack", "frontend"]));
    assert!(output.contains("Installed alpha -> .agents/skills/alpha"));
    assert!(output.contains("Installed beta -> .agents/skills/beta"));

    // Verify files on disk
    assert!(project.join(".agents/skills/alpha/SKILL.md").exists());
    assert!(project.join(".agents/skills/beta/SKILL.md").exists());
    assert_clean_project(&project);

    // Verify `skm pack status`
    let status_out = stdout_of(sandbox.skm_in(&project).args(["pack", "status"]));
    assert_eq!(
        table_cells(&status_out),
        rows(&[
            &["PACK", "SKILLS", "STATE"],
            &["frontend", "2 skills", "installed"],
        ])
    );

    // Verify individual `skm status`
    let ind_status = stdout_of(sandbox.skm_in(&project).args(["status"]));
    assert_eq!(
        table_cells(&ind_status),
        rows(&[
            &["SKILL", "TARGET", "STATE"],
            &["alpha", ".agents/skills", "ok"],
            &["beta", ".agents/skills", "ok"],
        ])
    );
}

#[test]
fn pack_add_via_pack_subcommand() {
    let (sandbox, _store, project) = ready();

    // Install pack using `skm pack add fullstack`
    let output = stdout_of(sandbox.skm_in(&project).args(["pack", "add", "fullstack"]));
    assert!(output.contains("Installed beta -> .agents/skills/beta"));
    assert!(output.contains("Installed gamma -> .agents/skills/gamma"));

    assert!(project.join(".agents/skills/beta/SKILL.md").exists());
    assert!(project.join(".agents/skills/gamma/SKILL.md").exists());
    assert_clean_project(&project);
}

#[test]
fn pack_multi_target_installation_and_removal() {
    let (sandbox, _store, project) = ready();

    // Install into multiple agents: opencode and pi
    let output = stdout_of(sandbox.skm_in(&project).args([
        "add",
        "--pack",
        "frontend",
        "--opencode",
        "--pi",
    ]));
    assert!(output.contains(".opencode/skills/alpha"));
    assert!(output.contains(".pi/skills/alpha"));
    assert!(output.contains(".opencode/skills/beta"));
    assert!(output.contains(".pi/skills/beta"));

    assert!(project.join(".opencode/skills/alpha").exists());
    assert!(project.join(".pi/skills/alpha").exists());
    assert_clean_project(&project);

    // Remove only from opencode
    let rm_output =
        stdout_of(
            sandbox
                .skm_in(&project)
                .args(["remove", "--pack", "frontend", "--opencode"]),
        );
    assert!(rm_output.contains("Removed alpha (.opencode/skills/alpha)"));
    assert!(rm_output.contains("Removed beta (.opencode/skills/beta)"));

    // Pi copy remains!
    assert!(!project.join(".opencode/skills/alpha").exists());
    assert!(project.join(".pi/skills/alpha").exists());
}

#[test]
fn pack_reference_counting_between_packs() {
    let (sandbox, _store, project) = ready();

    // Frontend has [alpha, beta]; Fullstack has [beta, gamma]
    stdout_of(sandbox.skm_in(&project).args(["add", "--pack", "frontend"]));
    stdout_of(
        sandbox
            .skm_in(&project)
            .args(["add", "--pack", "fullstack"]),
    );

    assert!(project.join(".agents/skills/alpha").exists());
    assert!(project.join(".agents/skills/beta").exists());
    assert!(project.join(".agents/skills/gamma").exists());

    // Remove frontend: alpha should be removed, beta should be RETAINED because of fullstack
    let rm_frontend = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "--pack", "frontend"]),
    );
    assert!(rm_frontend.contains("Removed alpha (.agents/skills/alpha)"));
    assert!(
        rm_frontend
            .contains("Retained beta (.agents/skills/beta; still required by pack 'fullstack')")
    );

    assert!(!project.join(".agents/skills/alpha").exists());
    assert!(project.join(".agents/skills/beta").exists());
    assert!(project.join(".agents/skills/gamma").exists());

    // Remove fullstack: now beta and gamma should both be removed
    let rm_fullstack = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "--pack", "fullstack"]),
    );
    assert!(rm_fullstack.contains("Removed beta (.agents/skills/beta)"));
    assert!(rm_fullstack.contains("Removed gamma (.agents/skills/gamma)"));

    assert!(!project.join(".agents/skills/beta").exists());
    assert!(!project.join(".agents/skills/gamma").exists());
    assert_clean_project(&project);
}

#[test]
fn pack_reference_counting_with_direct_install() {
    let (sandbox, _store, project) = ready();

    // Directly install alpha
    stdout_of(sandbox.skm_in(&project).args(["add", "alpha"]));

    // Now install frontend pack (which includes alpha and beta)
    stdout_of(sandbox.skm_in(&project).args(["add", "--pack", "frontend"]));

    // Removing frontend pack should retain alpha (since it was directly installed) and remove beta
    let rm_pack = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "--pack", "frontend"]),
    );
    assert!(
        rm_pack.contains("Retained alpha (.agents/skills/alpha; still required by direct install)")
    );
    assert!(rm_pack.contains("Removed beta (.agents/skills/beta)"));

    assert!(project.join(".agents/skills/alpha").exists());
    assert!(!project.join(".agents/skills/beta").exists());

    // Now directly removing alpha deletes it
    let rm_direct = stdout_of(sandbox.skm_in(&project).args(["remove", "alpha"]));
    assert!(rm_direct.contains("Removed alpha (.agents/skills/alpha)"));
    assert!(!project.join(".agents/skills/alpha").exists());
}

#[test]
fn pack_skill_removal_protects_against_accidental_delete() {
    let (sandbox, _store, project) = ready();

    // Install pack
    stdout_of(sandbox.skm_in(&project).args(["add", "--pack", "frontend"]));

    // Trying to remove a member skill directly without --force fails
    let (stdout, stderr) = failure_of(sandbox.skm_in(&project).args(["remove", "alpha"]));
    assert!(stdout.is_empty());
    assert!(stderr.contains("is part of pack 'frontend'"));

    // With --force, it removes it
    let forced = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "alpha", "--force"]),
    );
    assert!(forced.contains("Removed alpha (.agents/skills/alpha)"));
    assert!(!project.join(".agents/skills/alpha").exists());
}

#[test]
fn pack_locally_modified_member_detaches_during_sync_without_dropping_pack() {
    let (sandbox, _store, project) = ready();

    stdout_of(sandbox.skm_in(&project).args(["add", "--pack", "frontend"]));

    // Modify alpha locally
    let alpha_file = project.join(".agents/skills/alpha/SKILL.md");
    write(&alpha_file, "# Locally Modified Alpha\n");

    // Sync detaches alpha
    let sync_out = stdout_of(sandbox.skm_in(&project).args(["sync"]));
    assert!(sync_out.contains("detached  alpha -> .agents/skills/alpha (local modifications)"));

    // Status shows detached
    let status_out = stdout_of(sandbox.skm_in(&project).args(["status"]));
    assert!(status_out.contains("detached"));

    // Pack status still lists the pack!
    let pack_status = stdout_of(sandbox.skm_in(&project).args(["pack", "status"]));
    assert!(pack_status.contains("frontend"));

    // Removing the pack without --force refuses to delete modified alpha
    let (stdout, stderr) = failure_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "--pack", "frontend"]),
    );
    assert!(stdout.is_empty());
    assert!(stderr.contains("was modified in"));

    // With --force, pack is removed along with modified alpha
    let rm_force = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["remove", "--pack", "frontend", "--force"]),
    );
    assert!(rm_force.contains("Removed alpha"));
    assert!(rm_force.contains("Removed beta"));
    assert!(!project.join(".agents/skills/alpha").exists());
}

#[test]
fn pack_dry_run_writes_nothing() {
    let (sandbox, _store, project) = ready();

    let output =
        stdout_of(
            sandbox
                .skm_in(&project)
                .args(["add", "--pack", "frontend", "--dry-run"]),
        );
    assert!(output.contains("Would install alpha"));
    assert!(output.contains("Would install beta"));
    assert!(output.contains("Dry run: nothing was written."));

    // Verify nothing written
    assert!(!project.join(".agents").exists());
    assert_clean_project(&project);
}

#[test]
fn pack_edit_add_and_remove_skills() {
    let (sandbox, store, project) = ready();

    // 1. Add gamma to existing pack frontend (which had alpha, beta)
    let out_add = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["pack", "edit", "frontend", "--add", "gamma"]),
    );
    assert!(out_add.contains("Updated pack frontend (3 skills; added: gamma)"));

    // Verify via `skm pack show`
    let show_out = stdout_of(sandbox.skm_in(&project).args(["pack", "show", "frontend"]));
    assert!(show_out.contains("- alpha"));
    assert!(show_out.contains("- beta"));
    assert!(show_out.contains("- gamma"));

    // 2. Remove beta from frontend
    let out_rm = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["pack", "edit", "frontend", "--remove", "beta"]),
    );
    assert!(out_rm.contains("Updated pack frontend (2 skills; removed: beta)"));

    let show_after = stdout_of(sandbox.skm_in(&project).args(["pack", "show", "frontend"]));
    assert!(show_after.contains("- alpha"));
    assert!(!show_after.contains("- beta"));
    assert!(show_after.contains("- gamma"));

    // 3. Create a brand new pack by using --add
    let out_new = stdout_of(
        sandbox
            .skm_in(&project)
            .args(["pack", "edit", "brandnew", "--add", "alpha", "gamma"]),
    );
    assert!(out_new.contains("Updated pack brandnew (2 skills; added: alpha, gamma)"));

    assert!(store.join("packs/brandnew.toml").is_file());

    // 4. Reject adding a skill that does not exist in the store
    let (stdout, stderr) = failure_of(sandbox.skm_in(&project).args([
        "pack",
        "edit",
        "frontend",
        "--add",
        "nonexistent",
    ]));
    assert!(stdout.is_empty());
    assert!(stderr.contains("skill 'nonexistent' not found in the store"));
}
