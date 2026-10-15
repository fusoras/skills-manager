//! Skill discovery and metadata.
//!
//! A skill is a directory containing a `SKILL.md` file. That file may open with
//! a YAML frontmatter block declaring a `name`; otherwise the directory name is
//! used. M0 only needs `name`, so the parser is intentionally minimal and
//! dependency-free: no full YAML crate is pulled in.

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

/// File that marks a directory as a skill.
pub const SKILL_FILE: &str = "SKILL.md";

/// Frontmatter block delimiter.
const FRONTMATTER_DELIM: &str = "---";

/// Alternate frontmatter terminator (YAML document end).
const FRONTMATTER_END: &str = "...";

/// Maximum number of bytes read from a skill file when looking for frontmatter.
const MAX_SCAN_BYTES: u64 = 64 * 1024;

/// A skill found inside a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Effective name: the frontmatter `name`, else the directory name.
    pub name: String,
    /// Directory name on disk.
    pub dir_name: String,
    /// Skill directory relative to the store root, always `/` separated.
    pub rel_path: String,
    /// `true` when [`Skill::name`] came from the frontmatter.
    pub named: bool,
}

/// `true` when `dir` contains a `SKILL.md` file.
pub fn is_skill_dir(dir: &Path) -> bool {
    dir.join(SKILL_FILE).is_file()
}

/// Build a [`Skill`] from its directory, relative to `store_root`.
pub fn load(dir: &Path, store_root: &Path) -> Result<Skill> {
    let dir_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .with_context(|| format!("skill directory has no usable name: {}", dir.display()))?;
    let (name, named) = match read_declared_name(dir)? {
        Some(declared) => (declared, true),
        None => (dir_name.clone(), false),
    };
    Ok(Skill {
        name,
        dir_name,
        rel_path: relative_path(dir, store_root),
        named,
    })
}

/// Read the `name` declared in the frontmatter of `<dir>/SKILL.md`, if any.
pub fn read_declared_name(dir: &Path) -> Result<Option<String>> {
    let file = dir.join(SKILL_FILE);
    let mut handle =
        fs::File::open(&file).with_context(|| format!("opening {}", file.display()))?;
    let mut buffer = vec![0_u8; MAX_SCAN_BYTES as usize];
    let read = handle
        .read(&mut buffer)
        .with_context(|| format!("reading {}", file.display()))?;
    buffer.truncate(read);
    // Skill files are text, but a lossy read keeps a stray byte from failing the scan.
    let text = String::from_utf8_lossy(&buffer);
    Ok(frontmatter_name(&text))
}

/// Extract the `name` value from a leading YAML frontmatter block.
///
/// Only top-level scalar keys are considered; nested keys, quoted values and
/// `#` comments are handled, block scalars (`|`, `>`) are not supported and
/// yield `None` so that the directory name is used instead.
pub fn frontmatter_name(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    if lines.next()?.trim_end() != FRONTMATTER_DELIM {
        return None;
    }
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == FRONTMATTER_DELIM || trimmed == FRONTMATTER_END {
            break;
        }
        let Some((key, raw_value)) = split_key_value(line) else {
            continue;
        };
        if key == "name" {
            return clean_scalar(raw_value);
        }
    }
    None
}

/// Split a top-level `key: value` frontmatter line.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    if line.starts_with([' ', '\t', '#']) {
        return None;
    }
    let (key, value) = line.split_once(':')?;
    if key.is_empty() || key.contains([' ', '\t', '#']) {
        return None;
    }
    Some((key, value.trim()))
}

/// Normalize a frontmatter scalar: strip quotes, comments and block markers.
fn clean_scalar(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('|') || raw.starts_with('>') {
        return None;
    }
    if let Some(rest) = raw.strip_prefix('"') {
        return non_empty(&rest[..rest.find('"')?]);
    }
    if let Some(rest) = raw.strip_prefix('\'') {
        return non_empty(&rest[..rest.find('\'')?]);
    }
    match raw.find(" #") {
        Some(index) => non_empty(&raw[..index]),
        None => non_empty(raw),
    }
}

/// `Some(trimmed)` when `value` has content after trimming.
fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Path of `path` relative to `root`, `/` separated.
pub fn relative_path(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, contents: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(SKILL_FILE), contents).unwrap();
    }

    #[test]
    fn frontmatter_name_reads_a_plain_scalar() {
        let text = "---\nname: alpha\ndescription: first\n---\n\n# Alpha\n";
        assert_eq!(frontmatter_name(text).as_deref(), Some("alpha"));
    }

    #[test]
    fn frontmatter_name_requires_a_leading_delimiter() {
        assert_eq!(frontmatter_name("name: alpha\n"), None);
        assert_eq!(frontmatter_name(""), None);
        assert_eq!(frontmatter_name("\n---\nname: alpha\n---\n"), None);
    }

    #[test]
    fn frontmatter_name_accepts_document_end_terminator() {
        assert_eq!(
            frontmatter_name("---\nname: a\n...\nbody\n").as_deref(),
            Some("a")
        );
    }

    #[test]
    fn frontmatter_name_strips_quotes_and_comments() {
        assert_eq!(
            frontmatter_name("---\nname: \"quoted name\"\n---\n").as_deref(),
            Some("quoted name")
        );
        assert_eq!(
            frontmatter_name("---\nname: 'single'\n---\n").as_deref(),
            Some("single")
        );
        assert_eq!(
            frontmatter_name("---\nname: alpha # comment\n---\n").as_deref(),
            Some("alpha")
        );
        assert_eq!(
            frontmatter_name("---\nname: \"a # b\"\n---\n").as_deref(),
            Some("a # b")
        );
    }

    #[test]
    fn frontmatter_name_ignores_nested_and_commented_keys() {
        let text = "---\n# name: commented\nmeta:\n  name: nested\ndescription: x\n---\n";
        assert_eq!(frontmatter_name(text), None);
    }

    #[test]
    fn frontmatter_name_returns_none_for_empty_or_block_values() {
        assert_eq!(frontmatter_name("---\nname:\n---\n"), None);
        assert_eq!(frontmatter_name("---\nname: \"\"\n---\n"), None);
        assert_eq!(frontmatter_name("---\nname: >-\n  alpha\n---\n"), None);
        assert_eq!(frontmatter_name("---\nname: \"unterminated\n---\n"), None);
    }

    #[test]
    fn frontmatter_name_keeps_hash_without_leading_space() {
        assert_eq!(
            frontmatter_name("---\nname: a#b\n---\n").as_deref(),
            Some("a#b")
        );
    }

    #[test]
    fn frontmatter_name_stops_at_the_end_of_the_block() {
        let text = "---\ntitle: x\n---\nname: ignored\n";
        assert_eq!(frontmatter_name(text), None);
    }

    #[test]
    fn load_prefers_the_declared_name_over_the_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let dir = root.join("skills").join("dir-name");
        write(&dir, "---\nname: declared\n---\n");
        let skill = load(&dir, root).unwrap();
        assert_eq!(
            skill,
            Skill {
                name: "declared".to_owned(),
                dir_name: "dir-name".to_owned(),
                rel_path: "skills/dir-name".to_owned(),
                named: true,
            }
        );
    }

    #[test]
    fn load_falls_back_to_the_directory_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let dir = root.join("no-frontmatter");
        write(&dir, "# Just a title\n");
        let skill = load(&dir, root).unwrap();
        assert_eq!(skill.name, "no-frontmatter");
        assert_eq!(skill.dir_name, "no-frontmatter");
        assert_eq!(skill.rel_path, "no-frontmatter");
        assert!(!skill.named);
    }

    #[test]
    fn is_skill_dir_requires_the_skill_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("empty");
        fs::create_dir_all(&dir).unwrap();
        assert!(!is_skill_dir(&dir));
        write(&dir, "body");
        assert!(is_skill_dir(&dir));
    }

    #[test]
    fn read_declared_name_reports_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let error = read_declared_name(tmp.path()).unwrap_err();
        assert!(error.to_string().contains("opening"), "{error}");
    }
}
