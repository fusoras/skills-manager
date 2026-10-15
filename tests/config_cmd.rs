//! End-to-end tests for `skm config` and `skm --help`.

mod common;

use common::{DEFAULT_CONFIG, Sandbox, failure_of, stdout_of};

#[test]
fn config_path_prints_the_xdg_location_without_creating_it() {
    let sandbox = Sandbox::new();
    let expected = format!("{}\n", sandbox.config_file().display());

    sandbox
        .skm()
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(expected);

    assert!(
        !sandbox.config_file().exists(),
        "config path created a file"
    );
}
#[test]
fn config_show_prints_defaults_when_the_file_is_missing() {
    let sandbox = Sandbox::new();

    sandbox
        .skm()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(DEFAULT_CONFIG.to_owned());

    assert!(
        !sandbox.config_file().exists(),
        "config show created a file"
    );
}
#[test]
fn config_show_prints_the_file_verbatim_with_comments() {
    let sandbox = Sandbox::new();
    let original = "# my settings\n\
                    default_target = \".agents/skills\"\n\
                    store = \"~/skills-store\"  # keep\n\
                    \n\
                    [watch]\n\
                    debounce_ms = 250\n";
    sandbox.write_config(original);

    sandbox
        .skm()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(original.to_owned());
}
#[test]
fn config_show_rejects_a_corrupt_file() {
    let sandbox = Sandbox::new();
    sandbox.write_config("default_target = \nbroken");

    sandbox
        .skm()
        .args(["config", "show"])
        .assert()
        .failure()
        .stderr(predicates::str::starts_with(format!(
            "error: invalid config file {}: ",
            sandbox.config_file().display()
        )));
}
#[test]
fn commands_reject_the_old_named_stores_config() {
    let sandbox = Sandbox::new();

    // Every old shape is rejected on its own, not only in combination.
    for old in [
        "default_store = \"mine\"\n\n[stores.mine]\npath = \"~/skills-store\"\n",
        "default_store = \"mine\"\n",
        "[stores.mine]\npath = \"~/skills-store\"\n",
    ] {
        sandbox.write_config(old);
        for args in [vec!["config", "show"], vec!["store"], vec!["list"]] {
            let (_, stderr) = failure_of(sandbox.skm().args(&args));
            assert!(stderr.contains("old config format"), "{old}: {stderr}");
            assert!(stderr.contains("store = \"<path>\""), "{old}: {stderr}");
        }
    }
}
#[test]
fn help_works_without_touching_the_config() {
    let sandbox = Sandbox::new();

    let stdout = stdout_of(sandbox.skm().arg("--help"));

    assert!(stdout.starts_with("Manage skills"), "{stdout}");
    for command in ["config", "store", "list", "add", "remove", "status", "sync"] {
        assert!(stdout.contains(command), "help misses {command}: {stdout}");
    }

    let store_help = stdout_of(sandbox.skm().args(["store", "--help"]));
    for action in ["add", "show", "remove", "scan"] {
        assert!(
            store_help.contains(action),
            "store help misses {action}: {store_help}"
        );
    }

    assert!(!sandbox.config_file().exists(), "--help created the config");
}
