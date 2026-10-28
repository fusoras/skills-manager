//! Unit tests for rules and instruction blocks.

use super::*;
use tempfile::TempDir;

#[test]
fn wrap_in_markers_formats_correctly() {
    let untagged = wrap_in_markers("Rule text\n", None);
    assert_eq!(
        untagged,
        "<!-- BEGIN SKM INSTRUCTIONS -->\nRule text\n<!-- END SKM INSTRUCTIONS -->\n"
    );

    let tagged = wrap_in_markers("Rule text\n", Some("gemini"));
    assert_eq!(
        tagged,
        "<!-- BEGIN SKM INSTRUCTIONS: gemini -->\nRule text\n<!-- END SKM INSTRUCTIONS: gemini -->\n"
    );
}

#[test]
fn is_matching_begin_and_end_handles_tags() {
    assert!(is_matching_begin("<!-- BEGIN SKM INSTRUCTIONS -->", None));
    assert!(is_matching_begin(
        "<!--   BEGIN SKM INSTRUCTIONS   -->",
        None
    ));
    assert!(is_matching_begin(
        "<!-- BEGIN SKM INSTRUCTIONS: default -->",
        None
    ));
    assert!(is_matching_begin(
        "<!-- BEGIN SKM INSTRUCTIONS: gemini -->",
        Some("gemini")
    ));
    assert!(!is_matching_begin(
        "<!-- BEGIN SKM INSTRUCTIONS: gemini -->",
        Some("claude")
    ));

    assert!(is_matching_end("<!-- END SKM INSTRUCTIONS -->", None));
    assert!(is_matching_end("<!--   END SKM INSTRUCTIONS   -->", None));
    assert!(is_matching_end(
        "<!-- END SKM INSTRUCTIONS: default -->",
        None
    ));
    assert!(is_matching_end(
        "<!-- END SKM INSTRUCTIONS: gemini -->",
        Some("gemini")
    ));
    assert!(!is_matching_end(
        "<!-- END SKM INSTRUCTIONS: gemini -->",
        Some("claude")
    ));
}

#[test]
fn apply_to_empty_or_new_creates_marked_block() {
    let result = apply_block(None, "First instruction", None, false).unwrap();
    assert_eq!(
        result,
        "<!-- BEGIN SKM INSTRUCTIONS -->\nFirst instruction\n<!-- END SKM INSTRUCTIONS -->\n"
    );

    let from_empty = apply_block(Some(""), "First instruction", None, false).unwrap();
    assert_eq!(
        from_empty,
        "<!-- BEGIN SKM INSTRUCTIONS -->\nFirst instruction\n<!-- END SKM INSTRUCTIONS -->\n"
    );
}

#[test]
fn apply_preserves_prefix_and_suffix_text() {
    let existing = "# My Notes\nPersonal line 1\n\n<!-- BEGIN SKM INSTRUCTIONS -->\nOld instruction\n<!-- END SKM INSTRUCTIONS -->\n\n# Footer\nKeep this too\n";
    let updated = apply_block(Some(existing), "New instruction", None, false).unwrap();

    assert_eq!(
        updated,
        "# My Notes\nPersonal line 1\n\n<!-- BEGIN SKM INSTRUCTIONS -->\nNew instruction\n<!-- END SKM INSTRUCTIONS -->\n\n# Footer\nKeep this too\n"
    );
}

#[test]
fn apply_appends_to_unmarked_file_by_default() {
    let existing = "# User config\nDon't touch me\n";
    let updated = apply_block(Some(existing), "Appended instruction", None, false).unwrap();

    assert_eq!(
        updated,
        "# User config\nDon't touch me\n\n<!-- BEGIN SKM INSTRUCTIONS -->\nAppended instruction\n<!-- END SKM INSTRUCTIONS -->\n"
    );
}

#[test]
fn apply_overwrites_unmarked_file_when_flag_set() {
    let existing = "# User config\nOverwritten\n";
    let updated = apply_block(Some(existing), "New instruction", None, true).unwrap();

    assert_eq!(
        updated,
        "<!-- BEGIN SKM INSTRUCTIONS -->\nNew instruction\n<!-- END SKM INSTRUCTIONS -->\n"
    );
}

#[test]
fn apply_bails_on_corrupted_markers() {
    let missing_end = "# Notes\n<!-- BEGIN SKM INSTRUCTIONS -->\nInstruction without end\n";
    let err = apply_block(Some(missing_end), "New", None, false).unwrap_err();
    assert!(err.to_string().contains("without matching END marker"));

    let missing_begin = "# Notes\nInstruction without begin\n<!-- END SKM INSTRUCTIONS -->\n";
    let err2 = apply_block(Some(missing_begin), "New", None, false).unwrap_err();
    assert!(err2.to_string().contains("without matching BEGIN marker"));
}

#[test]
fn extract_block_retrieves_inner_content() {
    let content = "# Notes\n\n<!-- BEGIN SKM INSTRUCTIONS -->\nHello from store\n<!-- END SKM INSTRUCTIONS -->\n";
    let extracted = extract_block(content, None).unwrap().unwrap();
    assert_eq!(extracted, "Hello from store");

    let no_block = "# Notes without block\n";
    assert_eq!(extract_block(no_block, None).unwrap(), None);
}

#[test]
fn remove_block_cleans_file_when_only_block_present() {
    let content =
        "<!-- BEGIN SKM INSTRUCTIONS -->\nStore instruction\n<!-- END SKM INSTRUCTIONS -->\n";
    let remaining = remove_block(content, None).unwrap();
    assert_eq!(remaining, None);
}

#[test]
fn remove_block_preserves_surrounding_user_content() {
    let content = "# Header\nUser line\n\n<!-- BEGIN SKM INSTRUCTIONS -->\nStore instruction\n<!-- END SKM INSTRUCTIONS -->\n\n# Footer\nKeep me\n";
    let remaining = remove_block(content, None).unwrap().unwrap();
    assert_eq!(remaining, "# Header\nUser line\n\n# Footer\nKeep me\n");
}

#[test]
fn list_and_find_rules_in_store() {
    let tmp = TempDir::new().unwrap();
    let store_root = tmp.path();

    // 1. Create AGENTS.md in store root
    fs::write(store_root.join("AGENTS.md"), "# Root agents instructions\n").unwrap();

    // 2. Create rules/gemini.md
    fs::create_dir_all(store_root.join("rules")).unwrap();
    fs::write(
        store_root.join("rules").join("gemini.md"),
        "# Gemini specific\n",
    )
    .unwrap();

    let rules = list_rules(store_root).unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].name, "default");
    assert!(rules[0].is_default);
    assert_eq!(rules[1].name, "gemini");
    assert!(!rules[1].is_default);

    let default_rule = find_rule(store_root, "default").unwrap();
    assert_eq!(default_rule.name, "default");

    let agents_md_rule = find_rule(store_root, "AGENTS.md").unwrap();
    assert_eq!(agents_md_rule.name, "default");

    let gemini_rule = find_rule(store_root, "gemini").unwrap();
    assert_eq!(gemini_rule.name, "gemini");

    let gemini_file_rule = find_rule(store_root, "gemini.md").unwrap();
    assert_eq!(gemini_file_rule.name, "gemini");

    let gemini_path_rule = find_rule(store_root, "rules/gemini.md").unwrap();
    assert_eq!(gemini_path_rule.name, "gemini");
}

#[test]
fn resolve_targets_global_never_defaults_to_agents_agents_md() {
    let config = Config::default();
    let flags = RuleTargetFlags {
        global: true,
        ..RuleTargetFlags::default()
    };

    // Non-interactive without flags must fail with an error
    let err = resolve_rule_targets(&config, None, flags, None, false).unwrap_err();
    assert!(
        err.to_string()
            .contains("--global requires specifying a target agent")
    );

    // With --antigravity flag
    let agy_flags = RuleTargetFlags {
        global: true,
        antigravity: true,
        ..RuleTargetFlags::default()
    };
    let targets = resolve_rule_targets(&config, None, agy_flags, None, false).unwrap();
    assert_eq!(targets.len(), 1);
    assert!(targets[0].ends_with(".gemini/GEMINI.md"));

    // With --pi flag
    let pi_flags = RuleTargetFlags {
        global: true,
        pi: true,
        ..RuleTargetFlags::default()
    };
    let pi_targets = resolve_rule_targets(&config, None, pi_flags, None, false).unwrap();
    assert_eq!(pi_targets.len(), 1);
    assert!(pi_targets[0].ends_with(".pi/agent/AGENTS.md"));

    // With --copilot flag
    let copilot_flags = RuleTargetFlags {
        global: true,
        copilot: true,
        ..RuleTargetFlags::default()
    };
    let copilot_targets = resolve_rule_targets(&config, None, copilot_flags, None, false).unwrap();
    assert_eq!(copilot_targets.len(), 1);
    assert!(copilot_targets[0].ends_with(".copilot/copilot-instructions.md"));
}

#[test]
fn resolve_targets_local_defaults_to_agents_md() {
    let config = Config::default();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let flags = RuleTargetFlags::default();
    let targets = resolve_rule_targets(&config, Some(root), flags, None, false).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], root.join("AGENTS.md"));

    let pi_local = RuleTargetFlags {
        pi: true,
        ..RuleTargetFlags::default()
    };
    let pi_targets = resolve_rule_targets(&config, Some(root), pi_local, None, false).unwrap();
    assert_eq!(pi_targets.len(), 1);
    assert_eq!(pi_targets[0], root.join(".pi").join("AGENTS.md"));

    let copilot_local = RuleTargetFlags {
        copilot: true,
        ..RuleTargetFlags::default()
    };
    let copilot_targets =
        resolve_rule_targets(&config, Some(root), copilot_local, None, false).unwrap();
    assert_eq!(copilot_targets.len(), 1);
    assert_eq!(
        copilot_targets[0],
        root.join(".github").join("copilot-instructions.md")
    );
}
