//! Global configuration: model, tolerant loading and comment-preserving updates.
//!
//! The file lives at `<user config dir>/skm/config.toml` (on Linux
//! `~/.config/skm/config.toml` or `$XDG_CONFIG_HOME/skm/config.toml`). Reading
//! tolerates a missing file (defaults are used) but rejects malformed TOML with
//! a clear error. Writing goes through `toml_edit` so comments, key order and
//! formatting written by the user survive every mutation.
//!
//! The store is global and unique: it is named by its path and never by a label.
//!
//! ```toml
//! store = "~/folder-skills"
//! default_target = ".agents/skills"
//!
//! [agents]
//! opencode = ".opencode/skills"
//!
//! [watch]
//! debounce_ms = 500
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use toml_edit::DocumentMut;

use crate::doc;

/// Default project-relative directory where skills are installed.
pub const DEFAULT_TARGET: &str = ".agents/skills";

/// Directory created inside the user config dir.
pub const CONFIG_SUBDIR: &str = "skm";

/// Config file name.
pub const CONFIG_FILE: &str = "config.toml";

/// Error shown by every command that needs a registered store.
pub const NO_STORE: &str = "no store registered; run `skm store add <path>`";

/// Root configuration, mirroring the TOML file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Path of the global store, exactly as written by the user; `~` is
    /// expanded only when the store is used. Absent when none is registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
    /// Project-relative directory that skills are copied into.
    #[serde(default = "default_target")]
    pub default_target: String,
    /// Agent name to project-relative skills directory (used by later phases).
    #[serde(default = "default_agents")]
    pub agents: BTreeMap<String, String>,
    /// Watch settings (used by later phases).
    #[serde(default)]
    pub watch: Watch,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            store: None,
            default_target: default_target(),
            agents: default_agents(),
            watch: Watch::default(),
        }
    }
}

impl Config {
    /// Path of the global store as written, failing when none is registered.
    pub fn store_path(&self) -> Result<&str> {
        self.store.as_deref().context(NO_STORE)
    }
}

/// Watch settings (used by later phases).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    /// Debounce window for filesystem events, in milliseconds.
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce_ms(),
        }
    }
}

fn default_target() -> String {
    DEFAULT_TARGET.to_owned()
}

fn default_debounce_ms() -> u64 {
    500
}

fn default_agents() -> BTreeMap<String, String> {
    let mut agents = BTreeMap::new();
    agents.insert("opencode".to_owned(), ".opencode/skills".to_owned());
    agents
}

/// Path of the global config file, derived from the user config directory.
pub fn path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .context("cannot determine the user config directory (set XDG_CONFIG_HOME)")?;
    Ok(path_in(&base))
}

/// `<config_dir>/skm/config.toml` for an explicit config directory.
pub fn path_in(config_dir: &Path) -> PathBuf {
    config_dir.join(CONFIG_SUBDIR).join(CONFIG_FILE)
}

/// Load the global config; a missing file yields defaults.
pub fn load() -> Result<Config> {
    load_from(&path()?)
}

/// Load `path`; a missing file yields defaults, malformed TOML is an error.
pub fn load_from(path: &Path) -> Result<Config> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text).with_context(|| format!("invalid config file {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Parse config `text` into a [`Config`].
pub fn parse(text: &str) -> Result<Config> {
    let value: toml::Value = toml::from_str(text)?;
    reject_old_format(&value)?;
    value.try_into().map_err(Into::into)
}

/// Reject the pre-refactor format, where stores had names.
fn reject_old_format(value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    if table.contains_key("stores") || table.contains_key("default_store") {
        bail!(
            "old config format: the store is global now; replace `default_store` and the \
             `[stores]` table with a single `store = \"<path>\"` line"
        );
    }
    Ok(())
}

/// Read the raw config text, validating it; `None` when the file is missing.
pub fn read_raw(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            parse(&text).with_context(|| format!("invalid config file {}", path.display()))?;
            Ok(Some(text))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Rewrite the global config through `mutate`, preserving comments and layout.
pub fn update<F>(mutate: F) -> Result<()>
where
    F: FnOnce(&mut DocumentMut) -> Result<()>,
{
    update_at(&path()?, mutate)
}

/// Like [`update`], but against an explicit file.
pub fn update_at<F>(path: &Path, mutate: F) -> Result<()>
where
    F: FnOnce(&mut DocumentMut) -> Result<()>,
{
    let mut document = doc::read_document(path, "config file")?;
    mutate(&mut document)?;
    doc::write_document(path, &document)
}

/// Set the top-level `store` key, keeping every other key, comment and blank line.
pub fn doc_set_store(document: &mut DocumentMut, store_path: &str) {
    doc::set_string(document.as_table_mut(), "store", store_path);
}

/// Remove the top-level `store` key; returns whether it was there.
pub fn doc_remove_store(document: &mut DocumentMut) -> bool {
    document.remove("store").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn documents_dir() -> PathBuf {
        PathBuf::from("/tmp/config-home")
    }

    #[test]
    fn defaults_match_the_documented_shape() {
        let config = Config::default();
        assert_eq!(config.store, None);
        assert_eq!(config.default_target, ".agents/skills");
        assert_eq!(
            config.agents.get("opencode").map(String::as_str),
            Some(".opencode/skills")
        );
        assert_eq!(config.watch.debounce_ms, 500);
    }

    #[test]
    fn path_in_places_the_file_under_skm() {
        assert_eq!(
            path_in(&documents_dir()),
            PathBuf::from("/tmp/config-home/skm/config.toml")
        );
    }

    #[test]
    fn load_from_missing_file_returns_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("skm").join("config.toml");
        assert_eq!(load_from(&path).unwrap(), Config::default());
    }

    #[test]
    fn load_from_reads_the_store_and_the_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(
            &path,
            "store = \"~/folder-skills\"\n\
             default_target = \"custom/skills\"\n\
             \n\
             [agents]\n\
             opencode = \".opencode/skills\"\n\
             claude = \".claude/skills\"\n\
             \n\
             [watch]\n\
             debounce_ms = 900\n",
        )
        .unwrap();
        let config = load_from(&path).unwrap();
        assert_eq!(config.store.as_deref(), Some("~/folder-skills"));
        assert_eq!(config.default_target, "custom/skills");
        assert_eq!(
            config.agents.get("claude").map(String::as_str),
            Some(".claude/skills")
        );
        assert_eq!(config.watch.debounce_ms, 900);
    }

    #[test]
    fn store_path_fails_without_a_registered_store() {
        assert_eq!(
            Config::default().store_path().unwrap_err().to_string(),
            "no store registered; run `skm store add <path>`"
        );
        let config = Config {
            store: Some("~/store".to_owned()),
            ..Config::default()
        };
        assert_eq!(config.store_path().unwrap(), "~/store");
    }

    #[test]
    fn load_from_reports_corrupt_files_with_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(&path, "default_target = \nbroken").unwrap();
        let error = load_from(&path).unwrap_err();
        let message = format!("{error}");
        assert!(message.starts_with("invalid config file"), "{message}");
        assert!(message.contains("config.toml"), "{message}");
    }

    #[test]
    fn load_from_rejects_the_old_named_stores_format() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");

        for old in [
            "default_store = \"main\"\n",
            "[stores.main]\npath = \"~/store\"\n",
            "stores = 3\n",
        ] {
            fs::write(&path, old).unwrap();
            let error = load_from(&path).unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("old config format") && message.contains("store = \"<path>\""),
                "{message}"
            );
        }
    }

    #[test]
    fn read_raw_validates_and_returns_the_original_text() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        assert_eq!(read_raw(&path).unwrap(), None);

        let original = "# keep me\nstore = \"~/store\"\n";
        fs::write(&path, original).unwrap();
        assert_eq!(read_raw(&path).unwrap().as_deref(), Some(original));

        fs::write(&path, "= broken").unwrap();
        assert!(
            read_raw(&path)
                .unwrap_err()
                .to_string()
                .contains("invalid config file")
        );
    }

    #[test]
    fn default_config_roundtrips_through_toml() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(!text.contains("store"), "an absent store is not written");
        assert_eq!(parse(&text).unwrap(), Config::default());
    }

    #[test]
    fn update_at_creates_the_file_and_its_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("skm").join("config.toml");
        update_at(&path, |document| {
            doc_set_store(document, "~/folder-skills");
            Ok(())
        })
        .unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "store = \"~/folder-skills\"\n"
        );
        assert_eq!(
            load_from(&path).unwrap().store.as_deref(),
            Some("~/folder-skills")
        );
    }

    #[test]
    fn update_at_preserves_comments_blank_lines_and_key_order() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let original = "# Global settings\n\
                        default_target = \".agents/skills\"\n\
                        \n\
                        [agents]\n\
                        opencode = \".opencode/skills\"\n\
                        \n\
                        [watch]\n\
                        debounce_ms = 250\n";
        fs::write(&path, original).unwrap();

        let store_path = path.clone();
        update_at(&store_path, |document| {
            doc_set_store(document, "~/folder-skills");
            Ok(())
        })
        .unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# Global settings\n\
             default_target = \".agents/skills\"\n\
             store = \"~/folder-skills\"\n\
             \n\
             [agents]\n\
             opencode = \".opencode/skills\"\n\
             \n\
             [watch]\n\
             debounce_ms = 250\n"
        );
        let config = load_from(&path).unwrap();
        assert_eq!(config.store.as_deref(), Some("~/folder-skills"));
        assert_eq!(config.watch.debounce_ms, 250);
    }

    #[test]
    fn doc_set_store_keeps_the_comments_of_an_existing_key() {
        let mut document =
            "# my store\nstore = \"~/old\"  # trailing\n\n[watch]\ndebounce_ms = 250\n"
                .parse::<DocumentMut>()
                .unwrap();

        doc_set_store(&mut document, "~/new");

        assert_eq!(
            document.to_string(),
            "# my store\nstore = \"~/new\"  # trailing\n\n[watch]\ndebounce_ms = 250\n"
        );
    }

    #[test]
    fn doc_remove_store_drops_the_key_and_keeps_the_rest() {
        let mut document = "# header\ndefault_target = \"custom\"\nstore = \"/srv/store\"\n\n[watch]\ndebounce_ms = 250\n"
            .parse::<DocumentMut>()
            .unwrap();

        assert!(doc_remove_store(&mut document));
        assert!(!doc_remove_store(&mut document));

        assert_eq!(
            document.to_string(),
            "# header\ndefault_target = \"custom\"\n\n[watch]\ndebounce_ms = 250\n"
        );
    }
}
