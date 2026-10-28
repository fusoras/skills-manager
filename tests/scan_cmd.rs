//! End-to-end tests for the full store report (`skm list --all`).
//!
//! `skm store scan` remains as a hidden deprecated alias; it is also covered
//! here once to guarantee it still delegates to the same report.

mod common;

use std::fs;

use common::{Sandbox, lines};

/// Complete report output shared by `list --all` and the deprecated alias.
fn report_output(store: &std::path::Path) -> String {
    lines(&[
        "Store".to_owned(),
        format!("  path      {}", store.display()),
        "  state     OK".to_owned(),
        "  layout    flat".to_owned(),
        "  skills    2".to_owned(),
        "    - alpha-declared (alpha)".to_owned(),
        "    - beta (beta)".to_owned(),
        "  packs     2".to_owned(),
        "  warnings  2".to_owned(),
        "    ! alpha: frontmatter name 'alpha-declared' does not match directory name 'alpha'"
            .to_owned(),
        "    ! broken: directory without SKILL.md".to_owned(),
    ])
}

fn ready() -> (Sandbox, std::path::PathBuf) {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.register_store(&store);
    (sandbox, store)
}

#[test]
fn list_all_reports_the_full_store() {
    let (sandbox, store) = ready();

    sandbox
        .skm()
        .args(["list", "--all"])
        .assert()
        .success()
        .stdout(report_output(&store));
}
#[test]
fn store_scan_is_a_deprecated_alias_of_list_all() {
    let (sandbox, store) = ready();

    let assert = sandbox.skm().args(["store", "scan"]).assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("deprecated"), "{stderr}");
    assert!(stderr.contains("skm list --all"), "{stderr}");
    let stdout: String = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_eq!(stdout, report_output(&store));
}

#[test]
fn scan_reports_catalog_layout() {
    let sandbox = Sandbox::new();
    let store = sandbox.catalog_store();
    sandbox.register_store(&store);

    let expected = lines(&[
        "Store".to_owned(),
        format!("  path      {}", store.display()),
        "  state     OK".to_owned(),
        "  layout    catalog".to_owned(),
        "  skills    2".to_owned(),
        "    - delta (skills/delta)".to_owned(),
        "    - gamma (skills/gamma)".to_owned(),
        "  packs     0".to_owned(),
    ]);

    sandbox
        .skm()
        .args(["list", "--all"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn list_all_reports_a_missing_store() {
    let sandbox = Sandbox::new();
    let gone = sandbox.path("gone");
    fs::create_dir_all(&gone).expect("create dir");
    sandbox.register_store(&gone);
    fs::remove_dir_all(&gone).expect("remove dir");

    let expected = lines(&[
        "Store".to_owned(),
        format!("  path      {}", gone.display()),
        "  state     MISSING".to_owned(),
        "  layout    -".to_owned(),
        "  skills    0".to_owned(),
        "  packs     0".to_owned(),
        "  warnings  1".to_owned(),
        format!("    ! path does not exist: {}", gone.display()),
    ]);
    sandbox
        .skm()
        .args(["list", "--all"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn list_all_fails_without_a_registered_store() {
    let sandbox = Sandbox::new();

    sandbox
        .skm()
        .args(["list", "--all"])
        .assert()
        .failure()
        .stderr("error: no store registered; run `skm store add <path>`\n");
}
#[test]
fn list_all_takes_no_arguments() {
    let sandbox = Sandbox::new();

    sandbox
        .skm()
        .args(["list", "--all", "flat"])
        .assert()
        .failure()
        .code(2);
}
