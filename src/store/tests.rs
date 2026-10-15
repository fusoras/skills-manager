//! Unit tests for the store module (kept out of `store.rs` to keep it short).

use super::*;

fn skill_dir(dir: &Path, declared: Option<&str>) {
    fs::create_dir_all(dir).unwrap();
    let body = match declared {
        Some(name) => format!("---\nname: {name}\n---\n\n# Body\n"),
        None => "# No frontmatter\n".to_owned(),
    };
    fs::write(dir.join(skill::SKILL_FILE), body).unwrap();
}

/// The path of `store` as the user would have written it.
fn written(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn expand_tilde_with_expands_the_home_prefix() {
    let home = Path::new("/home/tester");
    assert_eq!(
        expand_tilde_with(Some(home), "~"),
        PathBuf::from("/home/tester")
    );
    assert_eq!(
        expand_tilde_with(Some(home), "~/skills-store"),
        PathBuf::from("/home/tester/skills-store")
    );
}

#[test]
fn expand_tilde_with_leaves_other_forms_untouched() {
    let home = Path::new("/home/tester");
    for input in [
        "~other/store",
        "/abs/store",
        "relative/store",
        "store",
        "",
        "~~/x",
    ] {
        assert_eq!(
            expand_tilde_with(Some(home), input),
            PathBuf::from(input),
            "input {input:?}"
        );
    }
    assert_eq!(expand_tilde_with(None, "~/x"), PathBuf::from("~/x"));
    assert_eq!(expand_tilde_with(None, "~"), PathBuf::from("~"));
}

#[test]
fn normalize_store_path_with_uses_the_tilde_form_inside_the_home() {
    let home = Path::new("/home/tester");
    let cwd = Path::new("/work");
    assert_eq!(
        normalize_store_path_with(Some(home), cwd, "~/skills-store"),
        "~/skills-store"
    );
    assert_eq!(
        normalize_store_path_with(Some(home), cwd, "/home/tester/skills-store"),
        "~/skills-store"
    );
    assert_eq!(normalize_store_path_with(Some(home), cwd, "~"), "~");
    assert_eq!(
        normalize_store_path_with(Some(home), cwd, "/home/tester"),
        "~"
    );
}

#[test]
fn normalize_store_path_with_resolves_relatives_against_the_working_directory() {
    let home = Path::new("/home/tester");
    assert_eq!(
        normalize_store_path_with(Some(home), Path::new("/home/tester"), "store"),
        "~/store"
    );
    assert_eq!(
        normalize_store_path_with(Some(home), Path::new("/work"), "./store"),
        "/work/store"
    );
    assert_eq!(
        normalize_store_path_with(Some(home), Path::new("/home/tester/projects"), "../store"),
        "~/store"
    );
}

#[test]
fn normalize_store_path_with_keeps_outside_paths_absolute() {
    let home = Path::new("/home/tester");
    let cwd = Path::new("/work");
    assert_eq!(
        normalize_store_path_with(Some(home), cwd, "/srv/store"),
        "/srv/store"
    );
    // A sibling of the home is not inside it, even with a shared prefix.
    assert_eq!(
        normalize_store_path_with(Some(home), cwd, "/home/tester-other/store"),
        "/home/tester-other/store"
    );
}

#[test]
fn detect_layout_requires_a_skills_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("store");
    fs::create_dir_all(&root).unwrap();
    assert_eq!(detect_layout(&root), Layout::Flat);

    fs::create_dir_all(root.join(SKILLS_SUBDIR)).unwrap();
    assert_eq!(detect_layout(&root), Layout::Catalog);

    assert_eq!(detect_layout(&tmp.path().join("missing")), Layout::Flat);
    // A `skills` *file* does not make a catalog.
    let other = tmp.path().join("file-store");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join(SKILLS_SUBDIR), "not a directory").unwrap();
    assert_eq!(detect_layout(&other), Layout::Flat);
}

#[test]
fn layout_as_str_is_stable() {
    assert_eq!(Layout::Flat.as_str(), "flat");
    assert_eq!(Layout::Catalog.as_str(), "catalog");
}

#[test]
fn skills_root_follows_the_layout() {
    let root = Path::new("/srv/store");
    assert_eq!(skills_root(root, Layout::Flat), PathBuf::from("/srv/store"));
    assert_eq!(
        skills_root(root, Layout::Catalog),
        PathBuf::from("/srv/store/skills")
    );
}

#[test]
fn scan_flat_layout_collects_skills_packs_and_warnings() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    skill_dir(&root.join("alpha"), Some("alpha-declared"));
    skill_dir(&root.join("beta"), None);
    skill_dir(&root.join(".hidden"), None);
    fs::create_dir_all(root.join("gamma")).unwrap();
    fs::write(root.join("loose-file.md"), "ignored").unwrap();
    fs::create_dir_all(root.join(PACKS_SUBDIR)).unwrap();
    fs::write(root.join(PACKS_SUBDIR).join("one.toml"), "").unwrap();
    fs::write(root.join(PACKS_SUBDIR).join("two.TOML"), "").unwrap();
    fs::write(root.join(PACKS_SUBDIR).join("notes.md"), "").unwrap();
    fs::write(root.join(PACKS_SUBDIR).join(".hidden.toml"), "").unwrap();

    let report = scan(&written(root)).unwrap();
    assert!(report.ready);
    assert_eq!(report.path_as_written, written(root));
    assert_eq!(report.resolved, root);
    assert_eq!(report.layout, Some(Layout::Flat));
    let names: Vec<&str> = report.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["alpha-declared", "beta"]);
    assert_eq!(report.skills[0].rel_path, "alpha");
    assert_eq!(report.skills[1].rel_path, "beta");
    assert_eq!(report.packs, 2);
    assert_eq!(
        report.warnings,
        [
            "alpha: frontmatter name 'alpha-declared' does not match directory name 'alpha'",
            "gamma: directory without SKILL.md",
        ]
    );
}

#[test]
fn scan_catalog_layout_only_looks_inside_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("loose-root-dir")).unwrap();
    skill_dir(&root.join(SKILLS_SUBDIR).join("alpha"), None);
    skill_dir(
        &root.join(SKILLS_SUBDIR).join("renamed"),
        Some("other-name"),
    );
    fs::create_dir_all(root.join(SKILLS_SUBDIR).join("broken")).unwrap();
    fs::create_dir_all(root.join(SKILLS_SUBDIR).join(".hidden")).unwrap();

    let report = scan(&written(root)).unwrap();
    assert_eq!(report.layout, Some(Layout::Catalog));
    assert_eq!(report.skills.len(), 2);
    assert_eq!(report.skills[0].rel_path, "skills/alpha");
    assert!(!report.skills[0].named);
    assert_eq!(report.skills[1].name, "other-name");
    assert_eq!(
        report.warnings,
        [
            "skills/broken: directory without SKILL.md",
            "skills/renamed: frontmatter name 'other-name' does not match directory name 'renamed'",
        ]
    );
    assert_eq!(report.packs, 0);
}

#[test]
fn scan_missing_path_is_reported_as_not_ready() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nope");
    let report = scan(&written(&missing)).unwrap();
    assert!(!report.ready);
    assert_eq!(report.layout, None);
    assert!(report.skills.is_empty());
    assert_eq!(
        report.warnings,
        [format!("path does not exist: {}", missing.display())]
    );
}

#[test]
fn scan_file_path_is_reported_as_not_a_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("store.toml");
    fs::write(&file, "").unwrap();
    let report = scan(&written(&file)).unwrap();
    assert!(!report.ready);
    assert_eq!(
        report.warnings,
        [format!("path is not a directory: {}", file.display())]
    );
}

#[test]
fn ensure_directory_creates_only_with_the_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nested").join("store");
    let written = missing.to_string_lossy().into_owned();

    let error = ensure_directory(&written, &missing, false)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        format!("path does not exist: {written} (pass --create to create it)")
    );
    assert!(!missing.exists());

    ensure_directory(&written, &missing, true).unwrap();
    assert!(missing.is_dir());

    ensure_directory(&written, &missing, false).unwrap();
}

#[test]
fn ensure_directory_rejects_files() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("store.toml");
    fs::write(&file, "").unwrap();
    let written = file.to_string_lossy().into_owned();

    let error = ensure_directory(&written, &file, true)
        .unwrap_err()
        .to_string();
    assert_eq!(error, format!("path is not a directory: {written}"));
}
