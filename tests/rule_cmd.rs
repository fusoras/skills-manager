mod common;

use std::fs;

use common::Sandbox;

fn setup_store_with_rules(sandbox: &Sandbox) -> std::path::PathBuf {
    let store = sandbox.flat_store();
    // Default AGENTS.md in store root
    fs::write(
        store.join("AGENTS.md"),
        "# Root Agent Guidelines\n- Always follow conventions\n",
    )
    .expect("write store AGENTS.md");

    // rules/review.md
    let rules_dir = store.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(
        rules_dir.join("review.md"),
        "# Code Review Checklist\n- Check edge cases\n",
    )
    .expect("write rules/review.md");

    sandbox.write_config(&format!("store = \"{}\"\n", store.display()));
    store
}

#[test]
fn rule_apply_creates_agents_md_in_project_with_markers() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    let agents_path = project.join("AGENTS.md");
    assert!(agents_path.is_file());
    let content = fs::read_to_string(&agents_path).expect("read AGENTS.md");

    assert!(content.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(content.contains("# Root Agent Guidelines"));
    assert!(content.contains("- Always follow conventions"));
    assert!(content.contains("<!-- END SKM INSTRUCTIONS -->"));
}

#[test]
fn rule_apply_non_destructive_preserves_surrounding_content() {
    let sandbox = Sandbox::new();
    let store = setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    let agents_path = project.join("AGENTS.md");
    fs::write(
        &agents_path,
        "# Project Custom Notes\nHere are custom team notes.\n",
    )
    .expect("write initial AGENTS.md");

    // Apply rule: should append marked block without wiping existing content
    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    let content = fs::read_to_string(&agents_path).expect("read AGENTS.md");
    assert!(content.starts_with("# Project Custom Notes\nHere are custom team notes.\n"));
    assert!(content.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(content.contains("# Root Agent Guidelines"));
    assert!(content.contains("<!-- END SKM INSTRUCTIONS -->"));

    // User adds notes below the block
    let appended = format!("{content}\n# Footer Notes\nDo not delete.\n");
    fs::write(&agents_path, &appended).expect("write modified AGENTS.md");

    // Update rule in store
    fs::write(
        store.join("AGENTS.md"),
        "# Root Agent Guidelines\n- Updated rule\n",
    )
    .expect("update store AGENTS.md");

    // Reapply / sync rule
    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    let updated_content = fs::read_to_string(&agents_path).expect("read AGENTS.md");
    assert!(updated_content.contains("# Project Custom Notes\nHere are custom team notes."));
    assert!(updated_content.contains("- Updated rule"));
    assert!(updated_content.contains("# Footer Notes\nDo not delete."));
}

#[test]
fn rule_apply_with_overwrite_replaces_entire_file() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    let agents_path = project.join("AGENTS.md");
    fs::write(&agents_path, "# Existing team text\n").expect("write AGENTS.md");

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply", "--overwrite"])
        .assert()
        .success();

    let content = fs::read_to_string(&agents_path).expect("read AGENTS.md");
    assert!(!content.contains("# Existing team text"));
    assert!(content.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(content.contains("# Root Agent Guidelines"));
}

#[test]
fn rule_apply_tagged_subrule() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply", "review"])
        .assert()
        .success();

    let agents_path = project.join("AGENTS.md");
    let content = fs::read_to_string(&agents_path).expect("read AGENTS.md");
    assert!(content.contains("<!-- BEGIN SKM INSTRUCTIONS: review -->"));
    assert!(content.contains("# Code Review Checklist"));
    assert!(content.contains("<!-- END SKM INSTRUCTIONS: review -->"));

    // Apply default rule as well to the same file
    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    let combined = fs::read_to_string(&agents_path).expect("read AGENTS.md");
    assert!(combined.contains("<!-- BEGIN SKM INSTRUCTIONS: review -->"));
    assert!(combined.contains("# Code Review Checklist"));
    assert!(combined.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(combined.contains("# Root Agent Guidelines"));
}

#[test]
fn rule_sync_reconciles_outdated_rules() {
    let sandbox = Sandbox::new();
    let store = setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    // Modify store rule
    fs::write(
        store.join("AGENTS.md"),
        "# Root Agent Guidelines\n- Newly synced rule\n",
    )
    .expect("update store AGENTS.md");

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "sync"])
        .assert()
        .success()
        .stdout(predicates::str::contains("updated"));

    let content = fs::read_to_string(project.join("AGENTS.md")).expect("read AGENTS.md");
    assert!(content.contains("- Newly synced rule"));
}

#[test]
fn rule_status_renders_table() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Project Rules:"))
        .stdout(predicates::str::contains("default"))
        .stdout(predicates::str::contains("AGENTS.md"))
        .stdout(predicates::str::contains("ok"));
}

#[test]
fn rule_list_and_show() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("default"))
        .stdout(predicates::str::contains("review"));

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "show", "review"])
        .assert()
        .success()
        .stdout(predicates::str::contains("# Code Review Checklist"));
}

#[test]
fn rule_remove_cleans_file_and_state() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);
    let project = sandbox.project();

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "apply"])
        .assert()
        .success();

    let agents_path = project.join("AGENTS.md");
    assert!(agents_path.is_file());

    sandbox
        .skm()
        .current_dir(&project)
        .args(["rule", "remove", "default"])
        .assert()
        .success();

    // Since the file only contained the marked block, it should be deleted
    assert!(!agents_path.exists());
}

#[test]
fn rule_global_without_target_fails_in_non_interactive_mode() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);

    // CRITICAL REQUIREMENT: Must not default to ~/.agents/AGENTS.md!
    // Must error requiring agent flag or --file.
    sandbox
        .skm()
        .args(["rule", "apply", "--global"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "--global requires specifying a target agent (--agy, --claude, --opencode, --cursor, --pi) or --file <path>",
        ));

    // Ensure ~/.agents/AGENTS.md was NEVER created
    assert!(!sandbox.home.join(".agents").join("AGENTS.md").exists());
}

#[test]
fn rule_global_with_agent_flag_installs_to_expected_agent_path() {
    let sandbox = Sandbox::new();
    setup_store_with_rules(&sandbox);

    sandbox
        .skm()
        .args(["rule", "apply", "--global", "--antigravity"])
        .assert()
        .success();

    let target = sandbox.home.join(".gemini").join("GEMINI.md");
    assert!(target.is_file());
    let content = fs::read_to_string(&target).expect("read GEMINI.md");
    assert!(content.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(content.contains("# Root Agent Guidelines"));

    // Global Pi target: ~/.pi/agent/AGENTS.md
    sandbox
        .skm()
        .args(["rule", "apply", "--global", "--pi"])
        .assert()
        .success();

    let pi_target = sandbox.home.join(".pi").join("agent").join("AGENTS.md");
    assert!(pi_target.is_file());
    let pi_content = fs::read_to_string(&pi_target).expect("read ~/.pi/agent/AGENTS.md");
    assert!(pi_content.contains("<!-- BEGIN SKM INSTRUCTIONS -->"));
    assert!(pi_content.contains("# Root Agent Guidelines"));
}
