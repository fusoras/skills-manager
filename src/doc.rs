//! Shared `toml_edit` helpers.
//!
//! Every file `skm` rewrites by hand (the global config and the project
//! manifest) goes through these helpers so comments, blank lines, key order and
//! value decorations written by the user survive.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

/// Parse `path` into an editable document; a missing file yields an empty one.
///
/// `what` names the file in error messages ("config file", "manifest", ...).
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

/// Borrow the top level array of tables `key`, creating it when absent.
pub fn array_of_tables_mut<'a>(
    document: &'a mut DocumentMut,
    key: &str,
) -> Result<&'a mut ArrayOfTables> {
    if !document.contains_key(key) {
        document.insert(key, Item::ArrayOfTables(ArrayOfTables::new()));
    }
    document[key]
        .as_array_of_tables_mut()
        .with_context(|| format!("`{key}` is not an array of tables"))
}

/// Set `key` to a string value, keeping the comments of an existing value.
pub fn set_string(table: &mut Table, key: &str, text: &str) {
    set_item(table, key, value(text));
}

/// Set `key` to a boolean value, keeping the comments of an existing value.
pub fn set_bool(table: &mut Table, key: &str, flag: bool) {
    set_item(table, key, value(flag));
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

/// Force `last_key` (a table or an array of tables) to render after everything
/// else in the document.
///
/// TOML parses a table that follows an array of tables as part of its last
/// element, so an array of tables must always be rendered last. `toml_edit`
/// renders tables by the position recorded while parsing rather than by
/// insertion order, so every top level item is given an explicit position here.
pub fn move_key_last(document: &mut DocumentMut, last_key: &str) {
    let others: Vec<String> = document
        .iter()
        .map(|(key, _)| key.to_owned())
        .filter(|name| name != last_key)
        .collect();

    let table = document.as_table_mut();
    let mut position: isize = 0;
    for name in others {
        if let Some(item) = table.get_mut(&name) {
            set_position(item, position);
        }
        position += 1;
    }
    if let Some(item) = table.get_mut(last_key) {
        set_position(item, position);
    }
}

/// Record the render position of a table, or of every element of an array of tables.
fn set_position(item: &mut Item, position: isize) {
    match item {
        Item::Table(table) => table.set_position(Some(position)),
        Item::ArrayOfTables(entries) => {
            for entry in entries.iter_mut() {
                entry.set_position(Some(position));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = "# header\n\
                            [table]\n\
                            # keep me\n\
                            key = \"old\"  # trailing\n\
                            other = 1\n";

    #[test]
    fn set_string_keeps_comments_and_neighbours() {
        let mut document = DOCUMENT.parse::<DocumentMut>().unwrap();
        set_string(document["table"].as_table_mut().unwrap(), "key", "new");

        assert_eq!(
            document.to_string(),
            "# header\n[table]\n# keep me\nkey = \"new\"  # trailing\nother = 1\n"
        );
    }

    #[test]
    fn set_string_appends_missing_keys() {
        let mut document = "".parse::<DocumentMut>().unwrap();
        let table = table_mut(&mut document, "defaults", false).unwrap();
        set_string(table, "target", ".agents/skills");

        assert_eq!(
            document.to_string(),
            "[defaults]\ntarget = \".agents/skills\"\n"
        );
    }

    #[test]
    fn set_bool_keeps_comments_and_replaces_the_value() {
        let mut document = "[install]\n# pin note\npin = true\n"
            .parse::<DocumentMut>()
            .unwrap();
        set_bool(document["install"].as_table_mut().unwrap(), "pin", false);

        assert_eq!(document.to_string(), "[install]\n# pin note\npin = false\n");
    }

    #[test]
    fn table_mut_is_idempotent_and_reports_broken_types() {
        let mut document = "[agents]\nopencode = \".opencode/skills\"\n"
            .parse::<DocumentMut>()
            .unwrap();
        let table = table_mut(&mut document, "agents", false).unwrap();
        set_string(table, "claude", ".claude/skills");
        assert_eq!(
            document.to_string(),
            "[agents]\nopencode = \".opencode/skills\"\nclaude = \".claude/skills\"\n"
        );

        let mut broken = "agents = 3\n".parse::<DocumentMut>().unwrap();
        let error = table_mut(&mut broken, "agents", false).unwrap_err();
        assert!(error.to_string().contains("is not a table"), "{error}");
    }

    #[test]
    fn implicit_tables_render_without_a_header() {
        let mut document = DocumentMut::new();
        let table = table_mut(&mut document, "agents", true).unwrap();
        table.insert("opencode", Item::Table(Table::new()));
        assert_eq!(document.to_string(), "[agents.opencode]\n");
    }

    #[test]
    fn array_of_tables_is_created_and_reports_broken_types() {
        let mut document = DocumentMut::new();
        let entries = array_of_tables_mut(&mut document, "install").unwrap();
        entries.push(Table::new());
        assert_eq!(document.to_string(), "[[install]]\n");

        let mut broken = "install = 3\n".parse::<DocumentMut>().unwrap();
        let error = array_of_tables_mut(&mut broken, "install").unwrap_err();
        assert!(
            error.to_string().contains("not an array of tables"),
            "{error}"
        );
    }

    #[test]
    fn move_key_last_puts_an_array_of_tables_at_the_end() {
        // Parsed tables carry a render position, so changing the order of the
        // map is not enough: the positions have to be rewritten.
        let mut document =
            "[[install]]\nskill = \"a\"\n\n[defaults]\ntarget = \".agents/skills\"\n"
                .parse::<DocumentMut>()
                .unwrap();

        move_key_last(&mut document, "install");

        let text = document.to_string();
        assert!(
            text.find("[defaults]").unwrap() < text.find("[[install]]").unwrap(),
            "{text}"
        );
        let reparsed: toml::Table = text.parse().unwrap();
        assert_eq!(
            reparsed["defaults"]["target"].as_str(),
            Some(".agents/skills")
        );
    }

    #[test]
    fn move_key_last_keeps_a_valid_document_after_adding_a_table() {
        use crate::manifest::Manifest;

        let mut document = "[[install]]\nskill = \"a\"\n"
            .parse::<DocumentMut>()
            .unwrap();
        let defaults = table_mut(&mut document, "defaults", false).unwrap();
        set_string(defaults, "target", ".agents/skills");
        move_key_last(&mut document, "install");

        let text = document.to_string();
        assert!(
            text.find("[defaults]").unwrap() < text.find("[[install]]").unwrap(),
            "{text}"
        );

        // The document must still mean the same thing after the reorder: a
        // table rendered after `[[install]]` would land inside its last element.
        let reloaded: Manifest = toml::from_str(&text).unwrap();
        assert_eq!(reloaded.install.len(), 1);
        assert_eq!(reloaded.install[0].skill.as_deref(), Some("a"));
        assert_eq!(reloaded.defaults.target.as_deref(), Some(".agents/skills"));
    }

    #[test]
    fn read_document_handles_missing_files_and_reports_corrupt_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("skills.toml");
        assert_eq!(read_document(&path, "manifest").unwrap().to_string(), "");

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "= broken").unwrap();
        let error = read_document(&path, "manifest").unwrap_err();
        assert!(error.to_string().starts_with("invalid manifest"), "{error}");
    }

    #[test]
    fn write_document_creates_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a").join("b").join("skills.toml");
        let document = "[defaults]\ntarget = \".agents/skills\"\n"
            .parse::<DocumentMut>()
            .unwrap();
        write_document(&path, &document).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[defaults]\ntarget = \".agents/skills\"\n"
        );
    }
}
