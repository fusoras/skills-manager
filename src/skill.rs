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
    /// Short description from frontmatter or SKILL.md, if any.
    pub description: Option<String>,
}

/// Metadata declared in a skill's file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillMeta {
    pub name: Option<String>,
    pub description: Option<String>,
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
    let meta = read_declared_metadata(dir)?;
    let (name, named) = match meta.name {
        Some(declared) => (declared, true),
        None => (dir_name.clone(), false),
    };
    Ok(Skill {
        name,
        dir_name,
        rel_path: relative_path(dir, store_root),
        named,
        description: meta.description,
    })
}

/// Read the metadata declared in `<dir>/SKILL.md`, if any.
pub fn read_declared_metadata(dir: &Path) -> Result<SkillMeta> {
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
    Ok(parse_metadata(&text))
}

/// Read the `name` declared in the frontmatter of `<dir>/SKILL.md`, if any.
#[cfg(test)]
pub fn read_declared_name(dir: &Path) -> Result<Option<String>> {
    Ok(read_declared_metadata(dir)?.name)
}

/// Extract metadata (name, description) from `contents`.
pub fn parse_metadata(contents: &str) -> SkillMeta {
    let mut meta = SkillMeta::default();
    let mut lines = contents.lines().peekable();

    if lines.peek().map(|l| l.trim_end()) == Some(FRONTMATTER_DELIM) {
        lines.next();
        while let Some(line) = lines.next() {
            let trimmed = line.trim_end();
            if trimmed == FRONTMATTER_DELIM || trimmed == FRONTMATTER_END {
                break;
            }
            if let Some((key, raw_value)) = split_key_value(line) {
                if key == "name" {
                    meta.name = clean_scalar(raw_value);
                } else if key == "description" {
                    let raw = raw_value.trim();
                    if raw.starts_with('>') || raw.starts_with('|') || raw.is_empty() {
                        let mut desc_lines = Vec::new();
                        while let Some(next_line) = lines.peek() {
                            let next_trimmed = next_line.trim_end();
                            if next_trimmed == FRONTMATTER_DELIM || next_trimmed == FRONTMATTER_END
                            {
                                break;
                            }
                            if !next_line.starts_with([' ', '\t']) {
                                break;
                            }
                            let l = lines.next().unwrap();
                            let content = l.trim();
                            if !content.is_empty() {
                                desc_lines.push(content);
                            }
                        }
                        if !desc_lines.is_empty() {
                            meta.description = Some(desc_lines.join(" "));
                        }
                    } else {
                        meta.description = clean_scalar(raw_value);
                    }
                }
            }
        }
    }

    if meta.description.is_none() {
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with("```")
                || trimmed == FRONTMATTER_DELIM
            {
                continue;
            }
            meta.description = non_empty(trimmed);
            break;
        }
    }

    if let Some(desc) = meta.description.take() {
        let norm = desc.split_whitespace().collect::<Vec<_>>().join(" ");
        if !norm.is_empty() {
            meta.description = Some(norm);
        }
    }

    meta
}

/// Extract the `name` value from a leading YAML frontmatter block.
#[cfg(test)]
pub fn frontmatter_name(contents: &str) -> Option<String> {
    parse_metadata(contents).name
}

/// Truncate `desc` to fit within `max_width`, appending `...` when truncated.
///
/// Under no circumstances is the first word hidden or cut off: even if `max_width`
/// is smaller than the first word, the first word is kept.
pub fn truncate_description(desc: &str, max_width: usize) -> String {
    let normalized = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return String::new();
    }

    let first_word = normalized.split_whitespace().next().unwrap_or("");

    if normalized.chars().count() <= max_width {
        return normalized;
    }

    let budget = max_width.saturating_sub(3).max(first_word.chars().count());

    if budget >= normalized.chars().count() {
        return normalized;
    }

    let truncated: String = normalized.chars().take(budget).collect();
    let trimmed = truncated.trim_end();
    format!("{trimmed}...")
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
                description: None,
            }
        );
    }

    #[test]
    fn parse_metadata_extracts_name_and_descriptions() {
        let text = "---\nname: my-skill\ndescription: A wonderful skill for testing\n---\n";
        let meta = parse_metadata(text);
        assert_eq!(meta.name.as_deref(), Some("my-skill"));
        assert_eq!(
            meta.description.as_deref(),
            Some("A wonderful skill for testing")
        );

        let folded = "---\nname: folded\ndescription: >\n  First line\n  second line\n---\n";
        let meta_folded = parse_metadata(folded);
        assert_eq!(
            meta_folded.description.as_deref(),
            Some("First line second line")
        );

        let body_fallback = "# Title\n\nThis is a fallback description from body.\n";
        let meta_body = parse_metadata(body_fallback);
        assert_eq!(
            meta_body.description.as_deref(),
            Some("This is a fallback description from body.")
        );
    }

    #[test]
    fn truncate_description_respects_max_width_and_first_word() {
        let desc = "Guide for writing idiomatic Rust code based on handbook";

        // Full fits
        assert_eq!(
            truncate_description(desc, 100),
            "Guide for writing idiomatic Rust code based on handbook"
        );

        // Truncated with ellipsis
        assert_eq!(truncate_description(desc, 25), "Guide for writing idio...");

        // Truncated close to first word
        assert_eq!(truncate_description(desc, 8), "Guide...");

        // First word limit: even when max_width is tiny, the first word is never hidden!
        assert_eq!(truncate_description(desc, 5), "Guide...");
        assert_eq!(truncate_description(desc, 2), "Guide...");

        // Single word description does not get ellipsis if it's already just the word
        assert_eq!(truncate_description("Single", 3), "Single");
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
