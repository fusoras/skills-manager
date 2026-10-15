//! End-to-end tests for `skm list`.

mod common;

use std::fs;

use common::{Sandbox, failure_of, rows, stdout_of, table_cells};

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
            &["SKILL", "PATH"],
            &["alpha-declared", "alpha"],
            &["beta", "beta"],
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
            &["SKILL", "PATH"],
            &["delta", "skills/delta"],
            &["gamma", "skills/gamma"],
        ])
    );
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
        .stdout("No skills found in the store.\n");
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
