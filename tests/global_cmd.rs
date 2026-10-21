//! End-to-end integration tests for global agent skills and removal.
//! Covers OpenCode (~/.config/opencode/skills), Antigravity (~/.gemini/skills),
//! Pi (~/.pi/agent/skills), status, sync, and safe agent-targeted removal.

mod common;

use std::path::PathBuf;

use common::{Sandbox, failure_of, is_symlink, read, stdout_of, write};

fn store(sandbox: &Sandbox, name: &str) -> PathBuf {
    let root = sandbox.path(name);
    sandbox.skill(&root, "alpha", Some("alpha"));
    write(&root.join("alpha/templates/base.md"), "template content\n");
    sandbox.skill(&root, "beta", Some("beta"));
    root
}

fn ready() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    let store = store(&sandbox, "store");
    sandbox.register_store(&store);
    (sandbox, store)
}

#[test]
fn global_add_opencode_installs_to_config_opencode_skills() {
    let (sandbox, _store) = ready();

    let output = stdout_of(
        sandbox
            .skm()
            .args(["add", "alpha", "--global", "--opencode"]),
    );
    assert_eq!(
        output,
        "Installed alpha -> ~/.config/opencode/skills/alpha\n"
    );

    let dest = sandbox.home.join(".config/opencode/skills/alpha");
    assert!(dest.is_dir(), "destination directory must exist");
    assert_eq!(
        read(&dest.join("SKILL.md")),
        "---\nname: alpha\ndescription: test skill\n---\n\n# Body\n"
    );
    assert_eq!(read(&dest.join("templates/base.md")), "template content\n");
    assert!(!is_symlink(&dest), "global copies must be real files");

    // Zero-pollution in projects, registered under [global]
    let registry = sandbox.registry_text();
    assert!(registry.contains("[global]") || registry.contains("[[global.skill]]"));
    assert!(registry.contains("~/.config/opencode/skills"));
}

#[test]
fn global_add_antigravity_installs_to_gemini_skills() {
    let (sandbox, _store) = ready();

    let output = stdout_of(sandbox.skm().args(["add", "alpha", "-g", "--agy"]));
    assert_eq!(output, "Installed alpha -> ~/.gemini/skills/alpha\n");

    let dest = sandbox.home.join(".gemini/skills/alpha");
    assert!(dest.is_dir());
    assert_eq!(
        read(&dest.join("SKILL.md")),
        "---\nname: alpha\ndescription: test skill\n---\n\n# Body\n"
    );

    let registry = sandbox.registry_text();
    assert!(registry.contains("~/.gemini/skills"));
}

#[test]
fn global_add_pi_installs_to_pi_agent_skills() {
    let (sandbox, _store) = ready();

    let output = stdout_of(sandbox.skm().args(["add", "alpha", "--global", "--pi"]));
    assert_eq!(output, "Installed alpha -> ~/.pi/agent/skills/alpha\n");

    let dest = sandbox.home.join(".pi/agent/skills/alpha");
    assert!(
        dest.is_dir(),
        "pi destination ~/.pi/agent/skills/alpha must exist"
    );
    assert_eq!(
        read(&dest.join("SKILL.md")),
        "---\nname: alpha\ndescription: test skill\n---\n\n# Body\n"
    );
}

#[test]
fn global_status_reports_global_skills() {
    let (sandbox, _store) = ready();

    stdout_of(sandbox.skm().args(["add", "alpha", "-g", "--opencode"]));
    stdout_of(sandbox.skm().args(["add", "beta", "-g", "--antigravity"]));

    let output = stdout_of(sandbox.skm().args(["status", "--global"]));
    assert!(output.contains("alpha"));
    assert!(output.contains("~/.config/opencode/skills"));
    assert!(output.contains("beta"));
    assert!(output.contains("~/.gemini/skills"));
    assert!(output.contains("ok"));
}

#[test]
fn global_remove_agent_specific_leaves_other_agents_untouched() {
    let (sandbox, _store) = ready();

    // Install alpha into both opencode and antigravity
    stdout_of(
        sandbox
            .skm()
            .args(["add", "alpha", "-g", "--opencode", "--antigravity"]),
    );

    let opencode_dest = sandbox.home.join(".config/opencode/skills/alpha");
    let agy_dest = sandbox.home.join(".gemini/skills/alpha");
    assert!(opencode_dest.is_dir());
    assert!(agy_dest.is_dir());

    // Remove only from opencode
    let output = stdout_of(
        sandbox
            .skm()
            .args(["remove", "alpha", "--global", "--opencode"]),
    );
    assert_eq!(output, "Removed alpha (~/.config/opencode/skills/alpha)\n");

    assert!(!opencode_dest.exists(), "opencode copy must be deleted");
    assert!(agy_dest.is_dir(), "antigravity copy must remain untouched");

    // Central registry should still keep antigravity record
    let registry = sandbox.registry_text();
    assert!(!registry.contains("~/.config/opencode/skills"));
    assert!(registry.contains("~/.gemini/skills"));
}

#[test]
fn global_remove_refuses_modified_copy_unless_forced() {
    let (sandbox, _store) = ready();

    stdout_of(sandbox.skm().args(["add", "alpha", "-g", "--opencode"]));
    let dest = sandbox.home.join(".config/opencode/skills/alpha");

    // Modify a file in the global directory
    write(&dest.join("SKILL.md"), "locally modified content\n");

    let (_stdout, stderr) = failure_of(sandbox.skm().args(["remove", "alpha", "-g", "--opencode"]));
    assert!(stderr.contains("was modified"));
    assert!(stderr.contains("--force"));
    assert!(
        dest.is_dir(),
        "modified directory must not be deleted without --force"
    );

    // With --force, deletion succeeds
    let output = stdout_of(
        sandbox
            .skm()
            .args(["remove", "alpha", "-g", "--opencode", "--force"]),
    );
    assert_eq!(output, "Removed alpha (~/.config/opencode/skills/alpha)\n");
    assert!(!dest.exists());
}

#[test]
fn global_sync_reconciles_changes_from_store() {
    let (sandbox, store) = ready();

    stdout_of(sandbox.skm().args(["add", "alpha", "-g", "--opencode"]));
    let dest = sandbox.home.join(".config/opencode/skills/alpha");
    assert_eq!(read(&dest.join("templates/base.md")), "template content\n");

    // Update store
    write(
        &store.join("alpha/templates/base.md"),
        "updated template v2\n",
    );

    // skm status --global shows behind
    let status_out = stdout_of(sandbox.skm().args(["status", "-g"]));
    assert!(status_out.contains("behind"));

    // skm sync --global updates it
    let sync_out = stdout_of(sandbox.skm().args(["sync", "--global"]));
    assert!(sync_out.contains("updated"));
    assert!(sync_out.contains("~/.config/opencode/skills/alpha"));

    assert_eq!(
        read(&dest.join("templates/base.md")),
        "updated template v2\n"
    );
}
