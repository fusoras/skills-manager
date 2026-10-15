//! End-to-end tests for `skm store`, `store add`, `store show` and `store remove`.

mod common;

use std::fs;

use common::{Sandbox, failure_of, lines};

#[test]
fn store_add_registers_the_path_and_show_reports_it() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();

    sandbox
        .skm()
        .args(["store", "add", &store.to_string_lossy()])
        .assert()
        .success()
        .stdout(format!("store = {}\n", store.display()));
    assert_eq!(
        sandbox.config_text(),
        format!("store = \"{}\"\n", store.display())
    );

    let expected = lines(&[
        "Store".to_owned(),
        format!("  path      {}", store.display()),
        "  state     OK".to_owned(),
        "  layout    flat".to_owned(),
        "  skills    2".to_owned(),
    ]);
    // Plain `skm store` is `skm store show`.
    sandbox
        .skm()
        .args(["store"])
        .assert()
        .success()
        .stdout(expected.clone());
    sandbox
        .skm()
        .args(["store", "show"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn store_add_of_the_same_path_is_a_no_op() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.register_store(&store);
    let before = sandbox.config_text();

    sandbox
        .skm()
        .args(["store", "add", &store.to_string_lossy()])
        .assert()
        .success()
        .stdout(format!("store already registered: {}\n", store.display()));

    assert_eq!(sandbox.config_text(), before);
}
#[test]
fn store_add_expands_tilde_and_keeps_the_path_as_written() {
    let sandbox = Sandbox::new();
    let absolute = sandbox.home.join("skills-store");

    sandbox
        .skm()
        .args(["store", "add", "~/skills-store", "--create"])
        .assert()
        .success()
        .stdout(format!(
            "store = ~/skills-store -> {}\n",
            absolute.display()
        ));
    assert!(absolute.is_dir(), "--create did not create the store");
    assert_eq!(sandbox.config_text(), "store = \"~/skills-store\"\n");

    let expected = lines(&[
        "Store".to_owned(),
        format!("  path      ~/skills-store -> {}", absolute.display()),
        "  state     OK".to_owned(),
        "  layout    flat".to_owned(),
        "  skills    0".to_owned(),
    ]);
    sandbox
        .skm()
        .args(["store", "show"])
        .assert()
        .success()
        .stdout(expected);

    // The same directory spelled in full is the same store: still a no-op.
    sandbox
        .skm()
        .args(["store", "add", &absolute.to_string_lossy()])
        .assert()
        .success()
        .stdout(format!(
            "store already registered: {}\n",
            absolute.display()
        ));
    assert_eq!(sandbox.config_text(), "store = \"~/skills-store\"\n");
}
#[test]
fn store_add_replaces_the_previous_store_and_mentions_it() {
    let sandbox = Sandbox::new();
    let flat = sandbox.flat_store();
    let catalog = sandbox.catalog_store();
    sandbox.register_store(&flat);

    sandbox
        .skm()
        .args(["store", "add", &catalog.to_string_lossy()])
        .assert()
        .success()
        .stdout(format!(
            "store = {} (was {})\n",
            catalog.display(),
            flat.display()
        ));

    assert_eq!(
        sandbox.config_text(),
        format!("store = \"{}\"\n", catalog.display())
    );
    assert!(flat.is_dir(), "the replaced store directory was deleted");
}
#[test]
fn store_add_requires_an_existing_directory_unless_create_is_given() {
    let sandbox = Sandbox::new();
    let missing = sandbox.path("missing").join("nested");

    sandbox
        .skm()
        .args(["store", "add", &missing.to_string_lossy()])
        .assert()
        .failure()
        .stderr(format!(
            "error: path does not exist: {} (pass --create to create it)\n",
            missing.display()
        ));
    assert!(!missing.exists(), "failed add created a directory");
    assert!(
        !sandbox.config_file().exists(),
        "failed add wrote the config"
    );

    sandbox
        .skm()
        .args(["store", "add", &missing.to_string_lossy(), "--create"])
        .assert()
        .success()
        .stdout(format!("store = {}\n", missing.display()));
    assert!(missing.is_dir(), "--create did not create the directory");
}
#[test]
fn store_add_rejects_files() {
    let sandbox = Sandbox::new();
    let file = sandbox.path("store.toml");
    fs::write(&file, "").expect("write file");

    sandbox
        .skm()
        .args(["store", "add", &file.to_string_lossy()])
        .assert()
        .failure()
        .stderr(format!(
            "error: path is not a directory: {}\n",
            file.display()
        ));
}
#[test]
fn store_remove_unregisters_the_store_and_keeps_the_rest() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.write_config(&format!(
        "# header\n\
         default_target = \"custom/target\"\n\
         store = \"{}\"  # mine\n\
         \n\
         [watch]\n\
         debounce_ms = 900\n",
        store.display()
    ));

    sandbox
        .skm()
        .args(["store", "remove"])
        .assert()
        .success()
        .stdout(format!("Removed store {}\n", store.display()));

    assert_eq!(
        sandbox.config_text(),
        "# header\ndefault_target = \"custom/target\"\n\n[watch]\ndebounce_ms = 900\n"
    );
    assert!(store.is_dir(), "remove deleted the store directory");

    let (_, stderr) = failure_of(sandbox.skm().args(["store", "remove"]));
    assert_eq!(
        stderr,
        "error: no store registered; run `skm store add <path>`\n"
    );
}
#[test]
fn store_show_reports_a_store_that_disappeared() {
    let sandbox = Sandbox::new();
    let store = sandbox.path("gone");
    fs::create_dir_all(&store).expect("create dir");
    sandbox.register_store(&store);
    fs::remove_dir_all(&store).expect("remove dir");

    let expected = lines(&[
        "Store".to_owned(),
        format!("  path      {}", store.display()),
        "  state     MISSING".to_owned(),
        "  layout    -".to_owned(),
        "  skills    0".to_owned(),
    ]);
    sandbox
        .skm()
        .args(["store"])
        .assert()
        .success()
        .stdout(expected);
}
#[test]
fn store_show_fails_without_a_registered_store() {
    let sandbox = Sandbox::new();

    for args in [vec!["store"], vec!["store", "show"]] {
        let (_, stderr) = failure_of(sandbox.skm().args(&args));
        assert_eq!(
            stderr,
            "error: no store registered; run `skm store add <path>`\n"
        );
    }
}
#[test]
fn store_add_preserves_comments_in_an_existing_config() {
    let sandbox = Sandbox::new();
    let store = sandbox.flat_store();
    sandbox.write_config(
        "# header comment\n\
         default_target = \".agents/skills\"\n\
         \n\
         [agents]\n\
         opencode = \".opencode/skills\"  # keep\n\
         \n\
         [watch]\n\
         debounce_ms = 900\n",
    );

    sandbox
        .skm()
        .args(["store", "add", &store.to_string_lossy()])
        .assert()
        .success();

    assert_eq!(
        sandbox.config_text(),
        format!(
            "# header comment\n\
             default_target = \".agents/skills\"\n\
             store = \"{}\"\n\
             \n\
             [agents]\n\
             opencode = \".opencode/skills\"  # keep\n\
             \n\
             [watch]\n\
             debounce_ms = 900\n",
            store.display()
        )
    );
}
