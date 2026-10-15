//! BLAKE3 hashing of skill trees.
//!
//! The hash of a tree is `blake3:` followed by the hex digest of one canonical
//! line per entry, sorted by line so the digest does not depend on the order the
//! filesystem returns entries in:
//!
//! ```text
//! <kind> <mode> <relative/path> <content-hash>
//! d - templates -
//! f - SKILL.md 8f1e...
//! f x bin/run 2a3c...
//! ```
//!
//! `kind` is `f` for files and `d` for directories, `mode` is `x` when the entry
//! has any execute bit and `-` otherwise, and `content-hash` is the BLAKE3 of
//! the file bytes (`-` for directories). Empty directories and the execute bit
//! are therefore part of the tree hash. Paths are relative to the hashed root
//! and always `/` separated, so the same content hashes the same anywhere.
//!
//! Names excluded by [`is_excluded`] are skipped here *and* by the copy in
//! [`crate::fsops::copy_tree`], which keeps `written_hash == source_hash` for a
//! freshly installed skill.

use std::ffi::OsStr;
use std::fs::{self, Metadata};
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::skill::relative_path;

/// Prefix of every hash produced here.
pub const HASH_PREFIX: &str = "blake3:";

/// Directory names skipped when hashing and copying a skill.
pub const EXCLUDED_DIRS: [&str; 2] = [".git", "node_modules"];

/// File names skipped when hashing and copying a skill.
pub const EXCLUDED_FILES: [&str; 1] = [".DS_Store"];

/// Suffix of temporary files that are skipped.
pub const EXCLUDED_SUFFIX: &str = ".tmp";

/// `true` for entries that are never hashed nor copied.
pub fn is_excluded(name: &OsStr) -> bool {
    if EXCLUDED_DIRS.iter().any(|dir| name == OsStr::new(dir)) {
        return true;
    }
    if EXCLUDED_FILES.iter().any(|file| name == OsStr::new(file)) {
        return true;
    }
    name.to_string_lossy().ends_with(EXCLUDED_SUFFIX)
}

/// Walk `root` deterministically, following symlinks and skipping excluded entries.
pub fn walk(root: &Path) -> impl Iterator<Item = walkdir::Result<DirEntry>> {
    WalkDir::new(root)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_excluded(entry.file_name()))
}

/// Hash the whole tree rooted at `root`.
pub fn tree_hash(root: &Path) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();

    for entry in walk(root) {
        let entry = entry.with_context(|| format!("walking {}", root.display()))?;
        let relative = relative_path(entry.path(), root);
        if relative.is_empty() {
            continue;
        }
        let metadata = entry
            .metadata()
            .with_context(|| format!("inspecting {}", entry.path().display()))?;
        if metadata.is_dir() {
            lines.push(format!("d {} {relative} -", mode(&metadata)));
        } else {
            let content = file_hash(entry.path())?;
            lines.push(format!("f {} {relative} {content}", mode(&metadata)));
        }
    }

    // The filesystem order is unspecified; the digest must not depend on it.
    lines.sort();

    let mut hasher = blake3::Hasher::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    Ok(format!("{HASH_PREFIX}{}", hasher.finalize().to_hex()))
}

/// BLAKE3 of a single file's bytes.
fn file_hash(path: &Path) -> Result<String> {
    let mut reader = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut reader, &mut hasher)
        .with_context(|| format!("hashing {}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// `x` when the entry is executable, `-` otherwise.
#[cfg(unix)]
fn mode(metadata: &Metadata) -> char {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o111 != 0 {
        'x'
    } else {
        '-'
    }
}

/// Platforms without an execute bit always report `-`.
#[cfg(not(unix))]
fn mode(_metadata: &Metadata) -> char {
    '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn hashes_carry_the_prefix_and_are_stable() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("SKILL.md"), "# Skill\n");

        let first = tree_hash(tmp.path()).unwrap();
        let second = tree_hash(tmp.path()).unwrap();
        assert!(first.starts_with(HASH_PREFIX), "{first}");
        assert_eq!(first, second);
        assert_eq!(first.len(), HASH_PREFIX.len() + 64);
    }

    #[test]
    fn content_changes_change_the_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("SKILL.md");
        write(&file, "# Skill\n");
        let before = tree_hash(tmp.path()).unwrap();

        write(&file, "# Skill!\n");
        assert_ne!(before, tree_hash(tmp.path()).unwrap());

        write(&tmp.path().join("extra.txt"), "extra");
        assert_ne!(before, tree_hash(tmp.path()).unwrap());
    }

    #[test]
    fn nested_paths_and_empty_directories_are_hashed() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("SKILL.md"), "# Skill\n");
        let before = tree_hash(tmp.path()).unwrap();

        fs::create_dir_all(tmp.path().join("templates").join("empty")).unwrap();
        let with_dir = tree_hash(tmp.path()).unwrap();
        assert_ne!(before, with_dir);

        // The same nested content in a differently named tree hashes the same.
        let other = tempfile::tempdir().unwrap();
        write(&other.path().join("SKILL.md"), "# Skill\n");
        fs::create_dir_all(other.path().join("templates").join("empty")).unwrap();
        assert_eq!(with_dir, tree_hash(other.path()).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn the_execute_bit_changes_the_hash() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("run.sh");
        write(&file, "#!/bin/sh\n");
        let plain = tree_hash(tmp.path()).unwrap();

        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = tree_hash(tmp.path()).unwrap();
        assert_ne!(plain, executable);

        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(plain, tree_hash(tmp.path()).unwrap());
    }

    #[test]
    fn excluded_entries_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("SKILL.md"), "# Skill\n");
        let baseline = tree_hash(tmp.path()).unwrap();

        write(&tmp.path().join(".git").join("config"), "junk");
        write(
            &tmp.path().join("node_modules").join("pkg").join("index.js"),
            "junk",
        );
        write(&tmp.path().join(".DS_Store"), "junk");
        write(&tmp.path().join("scratch.tmp"), "junk");

        assert_eq!(baseline, tree_hash(tmp.path()).unwrap());
    }

    #[test]
    fn is_excluded_matches_the_documented_names() {
        for excluded in [".git", "node_modules", ".DS_Store", "a.tmp"] {
            assert!(is_excluded(OsStr::new(excluded)), "{excluded}");
        }
        for kept in [".gitignore", "Skill.md", "tmp", "a.tmp.bak", ".hidden"] {
            assert!(!is_excluded(OsStr::new(kept)), "{kept}");
        }
    }
}
