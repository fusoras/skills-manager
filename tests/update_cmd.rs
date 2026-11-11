//! End-to-end tests for `skm update` and its aliases (`self-update`, `upgrade`).

mod common;

use common::Sandbox;

#[test]
fn update_help_shows_options_and_description() {
    let sandbox = Sandbox::new();

    let output = sandbox
        .skm()
        .args(["update", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("--check"), "missing --check flag in help");
    assert!(stdout.contains("--force"), "missing --force flag in help");
    assert!(stdout.contains("[TAG]"), "missing [TAG] argument in help");
}

#[test]
fn self_update_alias_shares_same_interface() {
    let sandbox = Sandbox::new();

    let output = sandbox
        .skm()
        .args(["self-update", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("--check"));
}

#[test]
fn upgrade_alias_shares_same_interface() {
    let sandbox = Sandbox::new();

    let output = sandbox
        .skm()
        .args(["upgrade", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("--check"));
}
