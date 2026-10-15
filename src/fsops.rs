//! Filesystem operations: real file copies (never symlinks) and atomic swaps.
//!
//! Installing a skill must never leave the destination half written, and must
//! never destroy a working copy when the copy fails, so every install goes
//! through a temporary directory next to the destination and a final `rename`
//! (atomic within one filesystem).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::hash;

/// Temporary directory used while `name` is installed in `parent`.
pub fn temp_dir(parent: &Path, name: &str) -> PathBuf {
    parent.join(format!(".skm-tmp-{name}"))
}

/// Directory holding the previous copy of `name` while it is replaced.
pub fn backup_dir(parent: &Path, name: &str) -> PathBuf {
    parent.join(format!(".skm-old-{name}"))
}

/// Remove `path`: a directory tree, a file, or nothing when it does not exist.
pub fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let outcome = if metadata.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            outcome.with_context(|| format!("removing {}", path.display()))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path.display())),
    }
}

/// Copy the tree at `source` into `dest` as real files, preserving permissions.
///
/// Symlinks are dereferenced (their content is copied) and the entries excluded
/// by [`hash::is_excluded`] are skipped, exactly like [`hash::tree_hash`] does.
pub fn copy_tree(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    copy_entries(source, dest)?;
    copy_permissions(source, dest)
}

fn copy_entries(source: &Path, dest: &Path) -> Result<()> {
    let entries = fs::read_dir(source).with_context(|| format!("reading {}", source.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", source.display()))?;
        let name = entry.file_name();
        if hash::is_excluded(&name) {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&name);
        // `metadata` follows symlinks: skills are copied as real files.
        let metadata =
            fs::metadata(&from).with_context(|| format!("inspecting {}", from.display()))?;
        if metadata.is_dir() {
            fs::create_dir_all(&to).with_context(|| format!("creating {}", to.display()))?;
            copy_permissions(&from, &to)?;
            copy_entries(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .with_context(|| format!("copying {} to {}", from.display(), to.display()))?;
            copy_permissions(&from, &to)?;
        }
    }
    Ok(())
}

/// Apply the permissions of `from` to `to`.
fn copy_permissions(from: &Path, to: &Path) -> Result<()> {
    let permissions = fs::metadata(from)
        .with_context(|| format!("inspecting {}", from.display()))?
        .permissions();
    fs::set_permissions(to, permissions)
        .with_context(|| format!("setting permissions on {}", to.display()))
}

/// Install the tree at `source` as `dest`, replacing it atomically.
///
/// The temp copy is removed when anything fails, and a previous `dest` is only
/// moved aside once the copy is complete, so a failure never leaves the
/// destination missing.
pub fn install_tree(source: &Path, dest: &Path) -> Result<()> {
    let parent = dest
        .parent()
        .with_context(|| format!("destination has no parent: {}", dest.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let name = dest
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("destination has no name: {}", dest.display()))?;
    let temp = temp_dir(parent, &name);
    let backup = backup_dir(parent, &name);

    // Leftovers from an interrupted run must never be reused.
    remove_path(&temp)?;
    remove_path(&backup)?;

    if let Err(error) = copy_tree(source, &temp) {
        let _ = remove_path(&temp);
        return Err(error);
    }

    let had_destination = fs::symlink_metadata(dest).is_ok();
    if had_destination {
        fs::rename(dest, &backup).with_context(|| format!("moving {} aside", dest.display()))?;
    }
    if let Err(error) = fs::rename(&temp, dest) {
        if had_destination {
            let _ = fs::rename(&backup, dest);
        }
        let _ = remove_path(&temp);
        return Err(error).with_context(|| format!("installing {}", dest.display()));
    }
    if had_destination {
        remove_path(&backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn copy_tree_copies_real_files_and_nested_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        write(&source.join("SKILL.md"), "# Skill\n");
        write(&source.join("templates").join("base.md"), "template\n");
        let dest = tmp.path().join("dest");

        copy_tree(&source, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "# Skill\n"
        );
        assert_eq!(
            fs::read_to_string(dest.join("templates").join("base.md")).unwrap(),
            "template\n"
        );
        assert!(dest.join("SKILL.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn copy_tree_preserves_permissions_and_writes_real_files() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let script = source.join("bin").join("run.sh");
        write(&script, "#!/bin/sh\n");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        write(&source.join("plain.md"), "x\n");
        fs::set_permissions(source.join("plain.md"), fs::Permissions::from_mode(0o600)).unwrap();

        // A symlinked file must be materialized, not recreated as a link.
        write(&source.join("target.md"), "linked\n");
        symlink(source.join("target.md"), source.join("link.md")).unwrap();

        let dest = tmp.path().join("dest");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        copy_tree(&source, &dest).unwrap();

        let mode_of =
            |path: &Path| fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode_of(&dest),
            0o700,
            "the copied root keeps the permissions of the source root"
        );
        assert_eq!(mode_of(&dest.join("bin")), mode_of(&source.join("bin")));
        assert_eq!(mode_of(&dest.join("bin").join("run.sh")), 0o755);
        assert_eq!(mode_of(&dest.join("plain.md")), 0o600);
        assert!(
            fs::symlink_metadata(dest.join("link.md"))
                .unwrap()
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(dest.join("link.md")).unwrap(),
            "linked\n"
        );
    }

    #[test]
    fn copy_tree_skips_excluded_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        write(&source.join("SKILL.md"), "# Skill\n");
        write(&source.join(".git").join("config"), "junk");
        write(
            &source.join("node_modules").join("pkg").join("index.js"),
            "junk",
        );
        write(&source.join(".DS_Store"), "junk");
        write(&source.join("scratch.tmp"), "junk");

        let dest = tmp.path().join("dest");
        copy_tree(&source, &dest).unwrap();

        assert_eq!(names(&dest), ["SKILL.md"]);
    }

    #[test]
    fn install_tree_replaces_previous_content_and_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        write(&source.join("SKILL.md"), "# New\n");
        let dest = tmp.path().join("target").join("skill");
        write(&dest.join("SKILL.md"), "# Old\n");
        write(&dest.join("stale.md"), "stale\n");

        install_tree(&source, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "# New\n"
        );
        assert!(!dest.join("stale.md").exists());
        assert_eq!(
            names(&tmp.path().join("target")),
            ["skill"],
            "temp or backup directories were left behind"
        );
    }

    #[test]
    fn install_tree_keeps_the_old_copy_when_the_source_is_unreadable() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("target").join("skill");
        write(&dest.join("SKILL.md"), "# Old\n");

        let error = install_tree(&tmp.path().join("missing"), &dest).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("reading"), "{message}");
        assert_eq!(
            fs::read_to_string(dest.join("SKILL.md")).unwrap(),
            "# Old\n"
        );
        assert_eq!(names(&tmp.path().join("target")), ["skill"]);
    }

    #[test]
    fn install_tree_creates_the_parent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        write(&source.join("SKILL.md"), "# New\n");
        let dest = tmp.path().join("a").join("b").join("skill");

        install_tree(&source, &dest).unwrap();

        assert!(dest.join("SKILL.md").is_file());
    }

    #[test]
    fn remove_path_handles_files_trees_and_absent_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("tree");
        write(&dir.join("nested").join("file.md"), "x");
        let file = tmp.path().join("file.md");
        write(&file, "x");

        remove_path(&file).unwrap();
        remove_path(&dir).unwrap();
        remove_path(&dir).unwrap();

        assert!(!file.exists());
        assert!(!dir.exists());
        assert_eq!(names(tmp.path()), Vec::<String>::new());
    }
}
