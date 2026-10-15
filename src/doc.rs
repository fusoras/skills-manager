//! Shared `toml_edit` helpers.
//!
//! Used by `config.rs` when updating the global configuration so comments,
//! blank lines, key order and value decorations written by the user survive.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, Table, value};

/// Parse `path` into an editable document; a missing file yields an empty one.
///
/// `what` names the file in error messages ("config file", ...).
pub fn read_document(path: &Path, what: &str) -> Result<DocumentMut> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .with_context(|| format!("invalid {what} {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Write `document` to `path`, creating parent directories as needed.
pub fn write_document(path: &Path, document: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, document.to_string()).with_context(|| format!("writing {}", path.display()))
}

/// Borrow the top level table `key`, creating it (implicitly on request) when absent.
#[cfg(test)]
pub fn table_mut<'a>(
    document: &'a mut DocumentMut,
    key: &str,
    implicit: bool,
) -> Result<&'a mut Table> {
    if !document.contains_key(key) {
        let mut table = Table::new();
        // Implicit tables render as `[parent.child]`, without a header of their own.
        table.set_implicit(implicit);
        document.insert(key, Item::Table(table));
    }
    document[key]
        .as_table_mut()
        .with_context(|| format!("`{key}` is not a table"))
}

/// Set `key` to a string value, keeping the comments of an existing value.
pub fn set_string(table: &mut Table, key: &str, text: &str) {
    set_item(table, key, value(text));
}

/// Set `key` to `item`, carrying over the comments of an existing value.
///
/// A plain `Table::insert` would replace the value item and drop the
/// `# comment` the user attached to it.
pub fn set_item(table: &mut Table, key: &str, item: Item) {
    if let Some(existing) = table.get_mut(key) {
        let mut fresh = item;
        let old_decor = existing
            .as_value()
            .map(|existing_value| existing_value.decor().clone());
        if let (Some(old_decor), Some(fresh_value)) = (old_decor, fresh.as_value_mut()) {
            *fresh_value.decor_mut() = old_decor;
        }
        *existing = fresh;
        return;
    }
    table.insert(key, item);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = "# header\n\
                            [table]\n\
                            # comment\n\
                            key = \"value\"\n";

    #[test]
    fn set_string_preserves_comments_when_key_exists() {
        let mut doc = DOCUMENT.parse::<DocumentMut>().unwrap();
        set_string(doc["table"].as_table_mut().unwrap(), "key", "updated");
        assert_eq!(
            doc.to_string(),
            "# header\n\
             [table]\n\
             # comment\n\
             key = \"updated\"\n"
        );
    }

    #[test]
    fn set_string_appends_when_key_is_missing() {
        let mut doc = DOCUMENT.parse::<DocumentMut>().unwrap();
        set_string(doc["table"].as_table_mut().unwrap(), "new_key", "fresh");
        assert_eq!(
            doc.to_string(),
            "# header\n\
             [table]\n\
             # comment\n\
             key = \"value\"\n\
             new_key = \"fresh\"\n"
        );
    }

    #[test]
    fn table_mut_creates_missing_table() {
        let mut document = DocumentMut::new();
        let table = table_mut(&mut document, "agents", true).unwrap();
        table.insert("opencode", Item::Table(Table::new()));
        assert_eq!(document.to_string(), "[agents.opencode]\n");
    }

    #[test]
    fn read_document_handles_missing_files_and_reports_corrupt_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("config.toml");
        assert_eq!(read_document(&path, "config").unwrap().to_string(), "");

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "= broken").unwrap();
        let error = read_document(&path, "config").unwrap_err();
        assert!(error.to_string().starts_with("invalid config"), "{error}");
    }

    #[test]
    fn write_document_creates_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("deep").join("config.toml");
        let mut document = DocumentMut::new();
        document["key"] = value("val");
        write_document(&path, &document).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "key = \"val\"\n");
    }
}
