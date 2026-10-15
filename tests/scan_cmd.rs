//! End-to-end tests for `skm store scan`.

mod common;

use std::fs;

use common::{Sandbox, lines};

#[test]
fn scan_reports_flat_layout_with_packs_and_warnings() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.register_store(&store);

    let expected = lines(&[
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
    ]);

    sandbox
        .skm()
        .args(["store", "scan"])
        .assert()
        .success()
        .stdout(expected);
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
        .args(["store", "scan"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn scan_reports_a_missing_store() {
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
        .args(["store", "scan"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn scan_fails_without_a_registered_store() {
    let sandbox = Sandbox::new();

    sandbox
        .skm()
        .args(["store", "scan"])
        .assert()
        .failure()
        .stderr("error: no store registered; run `skm store add <path>`\n");
}
#[test]
fn scan_takes_no_arguments() {
    let sandbox = Sandbox::new();

    sandbox
        .skm()
        .args(["store", "scan", "flat"])
        .assert()
        .failure()
        .code(2);
}
