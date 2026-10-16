//! Pack definition, discovery and parsing.
//!
//! A pack is a `<store>/packs/<name>.toml` file that names a collection of
//! skills that install together. Packs live in the store only: they are never
//! copied into projects and never exist as a file in them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::store::PACKS_SUBDIR;

/// A pack definition from `<store>/packs/<name>.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pack {
    /// Name of the pack (defaults to the file stem).
    #[serde(default)]
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Skill names that belong to this pack.
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Directory holding pack definitions inside `store_root`.
pub fn packs_dir(store_root: &Path) -> PathBuf {
    store_root.join(PACKS_SUBDIR)
}

/// Path to `<store_root>/packs/<name>.toml`.
pub fn pack_file(store_root: &Path, name: &str) -> PathBuf {
    packs_dir(store_root).join(format!("{name}.toml"))
}

/// Parse pack TOML `text` with a fallback `file_stem`.
pub fn parse(text: &str, file_stem: &str) -> Result<Pack> {
    let mut pack: Pack =
        toml::from_str(text).with_context(|| format!("invalid pack TOML in '{file_stem}.toml'"))?;
    if pack.name.trim().is_empty() {
        pack.name = file_stem.to_owned();
    }
    pack.skills.retain(|s| !s.trim().is_empty());
    pack.skills.sort();
    pack.skills.dedup();
    Ok(pack)
}

/// Load `<store_root>/packs/<name>.toml`.
pub fn load(store_root: &Path, name: &str) -> Result<Pack> {
    let file = pack_file(store_root, name);
    if !file.is_file() {
        // Try case-insensitive lookup if direct path doesn't exist
        if let Some(matched) = find_pack_file(store_root, name)? {
            let text = fs::read_to_string(&matched)
                .with_context(|| format!("reading pack file '{}'", matched.display()))?;
            let stem = matched
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.to_owned());
            return parse(&text, &stem);
        }
        bail!("pack '{name}' not found in the store packs/ directory");
    }

    let text = fs::read_to_string(&file)
        .with_context(|| format!("reading pack file '{}'", file.display()))?;
    parse(&text, name)
}

/// Find a pack file in `<store_root>/packs/` by case-insensitive name matching.
pub fn find_pack_file(store_root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let dir = packs_dir(store_root);
    if !dir.is_dir() {
        return Ok(None);
    }
    let lower_name = name.to_ascii_lowercase();
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_toml = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
        if !is_toml {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem.to_ascii_lowercase() == lower_name
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Ensure `<store_root>/packs/<name>.toml` exists, writing an initial template if not.
pub fn ensure_pack_template(store_root: &Path, name: &str) -> Result<PathBuf> {
    let dir = packs_dir(store_root);
    fs::create_dir_all(&dir).with_context(|| format!("creating directory {}", dir.display()))?;

    if let Some(matched) = find_pack_file(store_root, name)? {
        return Ok(matched);
    }

    let file = pack_file(store_root, name);
    if !file.is_file() {
        let template = format!(
            "# Pack definition for '{name}'\nname = \"{name}\"\ndescription = \"\"\nskills = [\n]\n"
        );
        fs::write(&file, template)
            .with_context(|| format!("writing pack file '{}'", file.display()))?;
    }
    Ok(file)
}

/// Add skills to a pack in `<store_root>/packs/<name>.toml`, creating the pack if it does not exist.
pub fn add_skills_to_pack(store_root: &Path, name: &str, skills_to_add: &[String]) -> Result<Pack> {
    let dir = packs_dir(store_root);
    fs::create_dir_all(&dir)?;

    let file = find_pack_file(store_root, name)?.unwrap_or_else(|| pack_file(store_root, name));

    let mut pack = if file.is_file() {
        load(store_root, name)?
    } else {
        Pack {
            name: name.to_owned(),
            description: None,
            skills: Vec::new(),
        }
    };

    for s in skills_to_add {
        if !pack.skills.contains(s) {
            pack.skills.push(s.clone());
        }
    }
    pack.skills.sort();
    pack.skills.dedup();

    save_pack_file(&file, &pack)?;
    Ok(pack)
}

/// Remove skills from a pack in `<store_root>/packs/<name>.toml`.
pub fn remove_skills_from_pack(
    store_root: &Path,
    name: &str,
    skills_to_remove: &[String],
) -> Result<Pack> {
    let file = find_pack_file(store_root, name)?.unwrap_or_else(|| pack_file(store_root, name));

    if !file.is_file() {
        bail!("pack '{name}' not found in the store packs/ directory");
    }

    let mut pack = load(store_root, name)?;
    pack.skills.retain(|s| !skills_to_remove.contains(s));

    save_pack_file(&file, &pack)?;
    Ok(pack)
}

fn save_pack_file(file: &Path, pack: &Pack) -> Result<()> {
    if file.is_file()
        && let Ok(content) = fs::read_to_string(file)
        && let Ok(mut doc) = content.parse::<toml_edit::DocumentMut>()
    {
        let mut arr = toml_edit::Array::new();
        for s in &pack.skills {
            arr.push(s.as_str());
        }
        doc["skills"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
        fs::write(file, doc.to_string())
            .with_context(|| format!("writing pack file '{}'", file.display()))?;
        return Ok(());
    }
    let toml_str = toml::to_string_pretty(pack)?;
    fs::write(file, toml_str).with_context(|| format!("writing pack file '{}'", file.display()))?;
    Ok(())
}

/// List all packs in `<store_root>/packs/`, sorted by name.
pub fn list(store_root: &Path) -> Result<Vec<Pack>> {
    let dir = packs_dir(store_root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut packs = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || !path.is_file() {
            continue;
        }
        let is_toml = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
        if !is_toml {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
        match fs::read_to_string(&path) {
            Ok(text) => match parse(&text, stem) {
                Ok(pack) => packs.push(pack),
                Err(_) => continue,
            },
            Err(_) => continue,
        }
    }

    packs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pack_with_all_fields() {
        let toml = r#"
name = "web"
description = "Web development pack"
skills = ["react", "tailwind", "seo"]
"#;
        let pack = parse(toml, "fallback").unwrap();
        assert_eq!(pack.name, "web");
        assert_eq!(pack.description.as_deref(), Some("Web development pack"));
        assert_eq!(pack.skills, ["react", "seo", "tailwind"]);
    }

    #[test]
    fn parse_pack_falls_back_to_stem() {
        let toml = r#"
skills = ["alpha", "beta"]
"#;
        let pack = parse(toml, "my-pack").unwrap();
        assert_eq!(pack.name, "my-pack");
        assert_eq!(pack.skills, ["alpha", "beta"]);
    }

    #[test]
    fn parse_pack_dedups_and_sorts_skills() {
        let toml = r#"
skills = ["zeta", "alpha", "zeta", ""]
"#;
        let pack = parse(toml, "pack").unwrap();
        assert_eq!(pack.skills, ["alpha", "zeta"]);
    }
}
