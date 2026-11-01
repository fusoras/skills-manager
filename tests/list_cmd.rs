//! End-to-end tests for `skm list`.

mod common;

use std::fs;

use common::{Sandbox, failure_of, rows, stdout_of, table_cells, write};

#[test]
fn list_shows_the_skills_of_the_store() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha-declared"));
    sandbox.skill(&store, "beta", None);
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[
            &["SKILL"],
            &["alpha-declared"],
            &["beta"],
            &[],
            &["Total: 2 skills"]
        ])
    );
}

#[test]
fn list_follows_the_catalog_layout() {
    let sandbox = Sandbox::new();
    let store = sandbox.catalog_store();
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[
            &["SKILL"],
            &["delta"],
            &["gamma"],
            &[],
            &["Total: 2 skills"]
        ])
    );
}

#[test]
fn list_shows_descriptions_with_flag() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha-declared"));
    sandbox.skill(&store, "beta", None);
    sandbox.register_store(&store);

    for flag in ["--desc", "--description", "-d"] {
        let listed = stdout_of(sandbox.skm().args(["list", flag]));
        assert_eq!(
            table_cells(&listed),
            rows(&[
                &["SKILL", "DESCRIPTION"],
                &["alpha-declared", "test skill"],
                &["beta", "-"],
                &[],
                &["Total: 2 skills"],
            ])
        );
    }
}

#[test]
fn list_truncates_long_description_with_ellipsis() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    let long_desc = "This is a very long description that exceeds standard terminal display limits and should be truncated with an ellipsis";
    sandbox.skill_with_desc(&store, "long-skill", "long-skill", long_desc);
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--desc"]));
    let cells = table_cells(&listed);
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[0], vec!["SKILL", "DESCRIPTION"]);
    assert_eq!(cells[1][0], "long-skill");
    assert!(cells[1][1].ends_with("..."));
    assert!(cells[1][1].len() <= 60);
    assert_eq!(cells[3], vec!["Total: 1 skill"]);
}

#[test]
fn list_adapts_to_terminal_width_via_columns() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    let desc = "Guide for writing idiomatic Rust code based on Apollo handbook";
    sandbox.skill_with_desc(&store, "rust-guide", "rust-guide", desc);
    sandbox.register_store(&store);

    // With COLUMNS=40, available description space is 40 - (10 + 2) = 28
    let listed_40 = stdout_of(sandbox.skm().env("COLUMNS", "40").args(["list", "--desc"]));
    let cells_40 = table_cells(&listed_40);
    let desc_40 = &cells_40[1][1];
    assert!(desc_40.ends_with("..."));
    assert!(desc_40.chars().count() <= 28);

    // With COLUMNS=25, available description space is 25 - (10 + 2) = 13
    let listed_25 = stdout_of(sandbox.skm().env("COLUMNS", "25").args(["list", "--desc"]));
    let cells_25 = table_cells(&listed_25);
    let desc_25 = &cells_25[1][1];
    assert!(desc_25.ends_with("..."));
    assert!(desc_25.chars().count() <= 13);
    // Smaller window cuts earlier
    assert!(desc_25.chars().count() < desc_40.chars().count());
}

#[test]
fn list_preserves_first_word_even_in_narrow_terminal() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    let desc = "Supercalifragilisticexpialidocious is a very long first word";
    sandbox.skill_with_desc(&store, "s", "s", desc);
    sandbox.register_store(&store);

    // Even with COLUMNS=5 (extreme narrow), the first word is never hidden!
    let listed = stdout_of(sandbox.skm().env("COLUMNS", "5").args(["list", "--desc"]));
    let cells = table_cells(&listed);
    assert_eq!(cells[1][1], "Supercalifragilisticexpialidocious...");
}
#[test]
fn list_reports_an_empty_store() {
    let sandbox = Sandbox::new();
    let empty = sandbox.path("empty-store");
    fs::create_dir_all(&empty).expect("create dir");
    sandbox.register_store(&empty);

    sandbox
        .skm()
        .args(["list"])
        .assert()
        .success()
        .stdout("No skills found in the store.\n\nTotal: 0 skills\n");
}

#[test]
fn list_skills_flag_reports_an_empty_store() {
    let sandbox = Sandbox::new();
    let empty = sandbox.path("empty-store");
    fs::create_dir_all(&empty).expect("create dir");
    sandbox.register_store(&empty);

    sandbox
        .skm()
        .args(["list", "--skills"])
        .assert()
        .success()
        .stdout("No skills found in the store.\n\nTotal: 0 skills\n");
}
#[test]
fn list_fails_without_a_registered_store() {
    let sandbox = Sandbox::new();

    let (_, stderr) = failure_of(sandbox.skm().args(["list"]));
    assert_eq!(
        stderr,
        "error: no store registered; run `skm store add <path>`\n"
    );
}
#[test]
fn list_fails_when_the_registered_store_is_gone() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.register_store(&store);
    fs::remove_dir_all(&store).expect("remove store dir");

    let (_, stderr) = failure_of(sandbox.skm().args(["list"]));
    assert_eq!(
        stderr,
        format!("error: path does not exist: {}\n", store.display())
    );
}

#[test]
fn list_shows_both_skills_and_packs_when_packs_exist() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.skill(&store, "beta", Some("beta"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        r#"
name = "bundle"
description = "A bundle of skills"
skills = ["alpha", "beta"]
"#,
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[
            &["SKILL"],
            &["alpha"],
            &["beta"],
            &[],
            &["Total: 2 skills"],
            &[],
            &["PACK", "SKILLS"],
            &["bundle", "alpha, beta"],
            &[],
            &["Total: 1 pack"],
        ])
    );
}

#[test]
fn list_shows_descriptions_for_both_skills_and_packs_with_desc_flag() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill_with_desc(&store, "alpha", "alpha", "First skill");
    sandbox.skill_with_desc(&store, "beta", "beta", "Second skill");

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        r#"
name = "bundle"
description = "A bundle of skills"
skills = ["alpha", "beta"]
"#,
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--desc"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[
            &["SKILL", "DESCRIPTION"],
            &["alpha", "First skill"],
            &["beta", "Second skill"],
            &[],
            &["Total: 2 skills"],
            &[],
            &["PACK", "SKILLS", "DESCRIPTION"],
            &["bundle", "alpha, beta", "A bundle of skills"],
            &[],
            &["Total: 1 pack"],
        ])
    );
}

#[test]
fn list_skills_flag_shows_only_skills() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        r#"
name = "bundle"
skills = ["alpha"]
"#,
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--skills"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[&["SKILL"], &["alpha"], &[], &["Total: 1 skill"]])
    );
}

#[test]
fn list_packs_flag_shows_only_packs_and_the_pack_total() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));
    sandbox.skill(&store, "beta", Some("beta"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        r#"
name = "bundle"
description = "A bundle of skills"
skills = ["alpha"]
"#,
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--packs"]));
    assert_eq!(
        table_cells(&listed),
        rows(&[
            &["PACK", "SKILLS", "DESCRIPTION"],
            &["bundle", "alpha", "A bundle of skills"],
            &[],
            &["Total: 1 pack"],
        ])
    );
}

#[test]
fn list_skills_flag_does_not_report_packs() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        "name = \"bundle\"\nskills = [\"alpha\"]\n",
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--skills"]));
    assert!(
        !listed.contains("pack"),
        "skills-only mode must not mention packs:\n{listed}"
    );
    assert!(
        listed.ends_with("\n\nTotal: 1 skill\n"),
        "unexpected output:\n{listed}"
    );
}

#[test]
fn list_packs_flag_does_not_report_skills() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("bundle.toml"),
        "name = \"bundle\"\nskills = [\"alpha\"]\n",
    );
    sandbox.register_store(&store);

    let listed = stdout_of(sandbox.skm().args(["list", "--packs"]));
    assert!(
        !listed.contains("skill"),
        "packs-only mode must not mention skills:\n{listed}"
    );
    assert!(
        listed.ends_with("\n\nTotal: 1 pack\n"),
        "unexpected output:\n{listed}"
    );
}

#[test]
fn list_reports_zero_pack_total_in_packs_mode_for_an_empty_store() {
    let sandbox = Sandbox::new();
    let empty = sandbox.path("empty-store");
    fs::create_dir_all(&empty).expect("create dir");
    sandbox.register_store(&empty);

    sandbox
        .skm()
        .args(["list", "--packs"])
        .assert()
        .success()
        .stdout("No packs found in the store.\n\nTotal: 0 packs\n");
}

#[test]
fn list_prints_each_total_right_below_its_own_table() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("store");
    sandbox.skill(&store, "alpha", Some("alpha"));

    let packs_dir = store.join("packs");
    fs::create_dir_all(&packs_dir).expect("create packs dir");
    write(
        &packs_dir.join("one.toml"),
        "name = \"one\"\nskills = [\"alpha\"]\n",
    );
    write(
        &packs_dir.join("two.toml"),
        "name = \"two\"\nskills = [\"alpha\"]\n",
    );
    sandbox.register_store(&store);

    // Default: the skills total sits between the two tables, the packs total closes the output.
    for args in [vec!["list"], vec!["list", "--desc"]] {
        let listed = stdout_of(sandbox.skm().args(&args));
        assert!(
            listed.contains("Total: 1 skill\n\nPACK"),
            "skills total is not right below the skills table for {args:?}:\n{listed}"
        );
        assert!(
            listed.ends_with("\n\nTotal: 2 packs\n"),
            "packs total is not right below the packs table for {args:?}:\n{listed}"
        );
        assert_eq!(
            listed.matches("Total:").count(),
            2,
            "expected one total per table for {args:?}:\n{listed}"
        );
    }

    // Single-table modes: only the total of the printed table.
    let skills_only = stdout_of(sandbox.skm().args(["list", "--skills"]));
    assert!(
        skills_only.ends_with("\n\nTotal: 1 skill\n"),
        "unexpected skills-only footer:\n{skills_only}"
    );
    assert_eq!(skills_only.matches("Total:").count(), 1);

    let packs_only = stdout_of(sandbox.skm().args(["list", "--packs"]));
    assert!(
        packs_only.ends_with("\n\nTotal: 2 packs\n"),
        "unexpected packs-only footer:\n{packs_only}"
    );
    assert_eq!(packs_only.matches("Total:").count(), 1);
}
