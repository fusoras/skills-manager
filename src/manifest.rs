//! Project manifest `skills.toml`: the committed declaration of what a project
//! installs.
//!
//! ```toml
//! [defaults]
//! target = ".agents/skills"
//!
//! [[install]]
//! skill = "git-commit"
//! pin = true
//! ```
//!
//! The store is global and unique (it lives in the global configuration), so a
//! declaration names only the skill or the pack it installs.
//!
//! Reading tolerates a missing file (an empty manifest) and rejects malformed
//! TOML. Writing goes through `toml_edit`, so comments and formatting survive.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use crate::doc;

/// Manifest model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// `[defaults]`.
    #[serde(default)]
    pub defaults: Defaults,
    /// `[[install]]` entries.
    #[serde(default, rename = "install", skip_serializing_if = "Vec::is_empty")]
    pub install: Vec<Install>,
}

/// `[defaults]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defaults {
    /// Project-relative directory that skills are installed into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// One `[[install]]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    /// Skill to install; mutually exclusive with `pack`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// Pack to install; not supported yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
    /// Keep the copy untouched by later syncs.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pin: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Install {
    /// Validate the entry shape.
    pub fn validate(&self) -> Result<()> {
        match (&self.skill, &self.pack) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(skill), Some(_)) => {
                bail!("install entry '{skill}' sets both `skill` and `pack`; use one of them")
            }
            (None, None) => bail!("install entry must set `skill` or `pack`"),
        }
    }
}

impl Manifest {
    /// Validate every entry: shape, duplicates and no unsupported kinds.
    pub fn validate(&self) -> Result<()> {
        let mut skills: Vec<&str> = Vec::new();
        let mut packs: Vec<&str> = Vec::new();
        for install in &self.install {
            install.validate()?;
            if let Some(skill) = install.skill.as_deref() {
                if skills.contains(&skill) {
                    bail!("duplicate install entry: skill '{skill}'");
                }
                skills.push(skill);
            }
            if let Some(pack) = install.pack.as_deref() {
                if packs.contains(&pack) {
                    bail!("duplicate install entry: pack '{pack}'");
                }
                packs.push(pack);
            }
        }
        Ok(())
    }

    /// Fail while `pack` entries cannot be installed nor reported on.
    pub fn reject_packs(&self) -> Result<()> {
        if let Some(install) = self.install.iter().find(|install| install.pack.is_some()) {
            bail!(
                "packs are not supported yet (install entry `pack = \"{}\"`)",
                install.pack.as_deref().unwrap_or_default()
            );
        }
        Ok(())
    }

    /// The declaration of `skill`, when the manifest has one.
    pub fn install_for(&self, skill: &str) -> Option<&Install> {
        self.install
            .iter()
            .find(|install| install.skill.as_deref() == Some(skill))
    }
}

/// Load the manifest at `path`; a missing file yields an empty manifest.
pub fn load(path: &Path) -> Result<Manifest> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let manifest =
                parse(&text).with_context(|| format!("invalid manifest {}", path.display()))?;
            manifest
                .validate()
                .with_context(|| format!("invalid manifest {}", path.display()))?;
            Ok(manifest)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Manifest::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Parse manifest `text` into a [`Manifest`].
pub fn parse(text: &str) -> Result<Manifest> {
    let value: toml::Value = toml::from_str(text)?;
    reject_old_format(&value)?;
    value.try_into().map_err(Into::into)
}

/// Reject the pre-refactor format, where the manifest mapped named stores.
fn reject_old_format(value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    let named_entry = table
        .get("install")
        .and_then(toml::Value::as_array)
        .is_some_and(|entries| entries.iter().any(|entry| entry.get("store").is_some()));
    if table.contains_key("stores") || named_entry {
        bail!(
            "old manifest format: the store is global now; remove the `[stores]` table and the \
             `store = \"...\"` line of every `[[install]]` entry"
        );
    }
    Ok(())
}

/// Add or refresh one `[[install]]` entry, preserving comments.
///
/// The entry of the same skill (or pack) is refreshed in place instead of being
/// duplicated, and `pin` is only ever turned on.
pub fn doc_add_install(document: &mut DocumentMut, install: &Install) -> Result<()> {
    doc::move_key_last(document, "install");
    let entries = doc::array_of_tables_mut(document, "install")?;
    for entry in entries.iter_mut() {
        if declares(entry, install) {
            if install.pin {
                doc::set_bool(entry, "pin", true);
            }
            return Ok(());
        }
    }

    let mut entry = Table::new();
    if let Some(skill) = install.skill.as_deref() {
        entry.insert("skill", toml_edit::value(skill));
    }
    if let Some(pack) = install.pack.as_deref() {
        entry.insert("pack", toml_edit::value(pack));
    }
    if install.pin {
        entry.insert("pin", toml_edit::value(true));
    }
    entries.push(entry);
    Ok(())
}

/// `true` when `entry` declares the same skill (or pack) as `install`.
fn declares(entry: &Table, install: &Install) -> bool {
    match (install.skill.as_deref(), install.pack.as_deref()) {
        (Some(skill), _) => entry.get("skill").and_then(Item::as_str) == Some(skill),
        (None, Some(pack)) => entry.get("pack").and_then(Item::as_str) == Some(pack),
        (None, None) => false,
    }
}

/// Remove the `[[install]]` entry of `skill`; 0 when absent.
pub fn doc_remove_install(document: &mut DocumentMut, skill: &str) -> usize {
    retain_installs(document, |entry| {
        entry.get("skill").and_then(Item::as_str) != Some(skill)
    })
}

/// Keep the entries `keep` accepts, dropping the rest; returns how many were dropped.
fn retain_installs<F: Fn(&Table) -> bool>(document: &mut DocumentMut, keep: F) -> usize {
    let Some(entries) = document
        .get_mut("install")
        .and_then(Item::as_array_of_tables_mut)
    else {
        return 0;
    };

    let mut kept = ArrayOfTables::new();
    let mut removed = 0;
    for entry in entries.iter() {
        if keep(entry) {
            kept.push(entry.clone());
        } else {
            removed += 1;
        }
    }
    *entries = kept;
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(skill: &str) -> Install {
        Install {
            skill: Some(skill.to_owned()),
            pack: None,
            pin: false,
        }
    }

    fn pack(name: &str) -> Install {
        Install {
            skill: None,
            pack: Some(name.to_owned()),
            pin: false,
        }
    }

    #[test]
    fn load_missing_manifest_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = load(&tmp.path().join("skills.toml")).unwrap();
        assert_eq!(manifest, Manifest::default());
        assert!(manifest.defaults.target.is_none());
    }

    #[test]
    fn load_reads_defaults_and_installs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("skills.toml");
        fs::write(
            &path,
            "[defaults]\ntarget = \".agents/skills\"\n\n\
             [[install]]\nskill = \"git-commit\"\npin = true\n\n\
             [[install]]\npack = \"web-stack\"\n",
        )
        .unwrap();

        let manifest = load(&path).unwrap();
        assert_eq!(manifest.defaults.target.as_deref(), Some(".agents/skills"));
        assert_eq!(manifest.install.len(), 2);
        assert_eq!(manifest.install[0].skill.as_deref(), Some("git-commit"));
        assert!(manifest.install[0].pin);
        assert_eq!(manifest.install[1].pack.as_deref(), Some("web-stack"));
        assert!(manifest.install[1].skill.is_none());
        // Pack entries parse fine; only consuming them is an error.
        manifest.validate().unwrap();
        assert_eq!(
            manifest.reject_packs().unwrap_err().to_string(),
            "packs are not supported yet (install entry `pack = \"web-stack\"`)"
        );
    }

    #[test]
    fn load_rejects_corrupt_manifests_and_declares_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("skills.toml");
        fs::write(&path, "[[install]]\nskill = \n").unwrap();

        let error = load(&path).unwrap_err().to_string();
        assert!(error.starts_with("invalid manifest"), "{error}");
        assert!(error.contains("skills.toml"), "{error}");
    }

    #[test]
    fn load_rejects_the_old_store_mapping() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("skills.toml");

        for old in [
            "[stores]\npersonal = \"~/store\"\n",
            "[[install]]\nskill = \"a\"\nstore = \"personal\"\n",
            "[stores]\npersonal = \"~/store\"\n\n[[install]]\npack = \"p\"\nstore = \"personal\"\n",
        ] {
            fs::write(&path, old).unwrap();
            let error = load(&path).unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("old manifest format")
                    && message.contains("remove the `[stores]` table"),
                "{message}"
            );
        }
    }

    #[test]
    fn duplicates_and_malformed_entries_are_rejected() {
        let duplicate_skill = Manifest {
            install: vec![install("a"), install("a")],
            ..Manifest::default()
        };
        assert_eq!(
            duplicate_skill.validate().unwrap_err().to_string(),
            "duplicate install entry: skill 'a'"
        );

        let duplicate_pack = Manifest {
            install: vec![pack("p"), pack("p")],
            ..Manifest::default()
        };
        assert_eq!(
            duplicate_pack.validate().unwrap_err().to_string(),
            "duplicate install entry: pack 'p'"
        );

        let both = Manifest {
            install: vec![Install {
                skill: Some("a".to_owned()),
                pack: Some("p".to_owned()),
                pin: false,
            }],
            ..Manifest::default()
        };
        assert_eq!(
            both.validate().unwrap_err().to_string(),
            "install entry 'a' sets both `skill` and `pack`; use one of them"
        );

        let neither = Manifest {
            install: vec![Install {
                skill: None,
                pack: None,
                pin: false,
            }],
            ..Manifest::default()
        };
        assert_eq!(
            neither.validate().unwrap_err().to_string(),
            "install entry must set `skill` or `pack`"
        );
    }

    #[test]
    fn install_for_finds_the_declaration() {
        let manifest = Manifest {
            install: vec![install("a"), install("b")],
            ..Manifest::default()
        };

        assert!(manifest.install_for("a").is_some());
        assert!(manifest.install_for("b").is_some());
        assert!(manifest.install_for("missing").is_none());
    }

    #[test]
    fn doc_add_install_appends_entries_with_the_pin_flag() {
        let mut document = DocumentMut::new();
        doc_add_install(&mut document, &install("a")).unwrap();
        doc_add_install(
            &mut document,
            &Install {
                pin: true,
                ..install("b")
            },
        )
        .unwrap();

        assert_eq!(
            document.to_string(),
            "[[install]]\nskill = \"a\"\n\n\
             [[install]]\nskill = \"b\"\npin = true\n"
        );
    }

    #[test]
    fn doc_add_install_refreshes_instead_of_duplicating() {
        let mut document =
            "# comment\n[[install]]\nskill = \"a\"\n\n[defaults]\ntarget = \".agents/skills\"\n"
                .parse::<DocumentMut>()
                .unwrap();

        doc_add_install(
            &mut document,
            &Install {
                pin: true,
                ..install("a")
            },
        )
        .unwrap();

        let text = document.to_string();
        assert_eq!(text.matches("skill = \"a\"").count(), 1, "{text}");
        assert!(
            text.contains("[[install]]\nskill = \"a\"\npin = true\n"),
            "{text}"
        );
        let reparsed: Manifest = toml::from_str(&text).unwrap();
        assert_eq!(reparsed.install.len(), 1);
        assert!(reparsed.install[0].pin);
        assert!(text.find("[defaults]").unwrap() < text.find("[[install]]").unwrap());
    }

    #[test]
    fn doc_remove_install_drops_only_the_matching_entry() {
        let mut document = "[[install]]\nskill = \"a\"\n\n\
                            [[install]]\nskill = \"b\"\n"
            .parse::<DocumentMut>()
            .unwrap();

        assert_eq!(doc_remove_install(&mut document, "a"), 1);
        assert_eq!(doc_remove_install(&mut document, "a"), 0);

        let reparsed: Manifest = toml::from_str(&document.to_string()).unwrap();
        assert_eq!(reparsed.install.len(), 1);
        assert_eq!(reparsed.install[0].skill.as_deref(), Some("b"));
    }
}
