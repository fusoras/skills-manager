//! Project layout: where `skills.toml` and `.skills-manager/state.toml` live.
//!
//! A project is the nearest directory at or above the working directory that
//! holds `skills.toml` or `.skills-manager/`; when there is none, the working
//! directory itself becomes the project root, and `skm add` creates the
//! manifest there.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::DocumentMut;

use crate::doc;
use crate::manifest::{self, Manifest};
use crate::state::{self, State};

/// Committed manifest file name.
pub const MANIFEST_FILE: &str = "skills.toml";

/// Local state directory name (never committed).
pub const STATE_DIR: &str = ".skills-manager";

/// Local state file name.
pub const STATE_FILE: &str = "state.toml";

/// `skills.toml` inside `root`.
pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_FILE)
}

/// `.skills-manager/state.toml` inside `root`.
pub fn state_path(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(STATE_FILE)
}

/// Project root of the current working directory.
pub fn root() -> Result<PathBuf> {
    let start = std::env::current_dir().context("cannot determine the working directory")?;
    Ok(root_from(&start))
}

/// [`root`] for an explicit starting directory.
pub fn root_from(start: &Path) -> PathBuf {
    for candidate in start.ancestors() {
        if candidate.join(MANIFEST_FILE).is_file() || candidate.join(STATE_DIR).is_dir() {
            return candidate.to_path_buf();
        }
    }
    start.to_path_buf()
}

/// An open project: its root, manifest and local state.
pub struct Project {
    root: PathBuf,
    /// Committed declaration read from `skills.toml`.
    pub manifest: Manifest,
    /// Local install state read from `.skills-manager/state.toml`.
    pub state: State,
}

impl Project {
    /// Open the project containing the working directory.
    pub fn open() -> Result<Self> {
        Self::open_at(&root()?)
    }

    /// [`Project::open`] for an explicit root.
    pub fn open_at(root: &Path) -> Result<Self> {
        Ok(Self {
            manifest: manifest::load(&manifest_path(root))?,
            state: state::load(&state_path(root))?,
            root: root.to_path_buf(),
        })
    }

    /// Project root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path of `skills.toml`.
    pub fn manifest_path(&self) -> PathBuf {
        manifest_path(&self.root)
    }

    /// Path of `.skills-manager/state.toml`.
    pub fn state_path(&self) -> PathBuf {
        state_path(&self.root)
    }

    /// Load the manifest as an editable document.
    pub fn document(&self) -> Result<DocumentMut> {
        doc::read_document(&self.manifest_path(), "manifest")
    }

    /// Write the manifest document back, creating the file when needed.
    pub fn write_document(&self, document: &DocumentMut) -> Result<()> {
        doc::write_document(&self.manifest_path(), document)
    }

    /// Persist the state file.
    pub fn save_state(&self) -> Result<()> {
        state::save(&self.state_path(), &self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_rooted_in_the_project() {
        let root = Path::new("/work/project");
        assert_eq!(
            manifest_path(root),
            PathBuf::from("/work/project/skills.toml")
        );
        assert_eq!(
            state_path(root),
            PathBuf::from("/work/project/.skills-manager/state.toml")
        );
    }

    #[test]
    fn root_from_walks_up_to_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        let nested = root.join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            manifest_path(&root),
            "[defaults]\ntarget = \".agents/skills\"\n",
        )
        .unwrap();

        assert_eq!(root_from(&nested), root);
        assert_eq!(root_from(&root), root);
        // Outside the project the starting directory is the root.
        assert_eq!(root_from(tmp.path()), tmp.path());
    }

    #[test]
    fn root_from_walks_up_to_the_state_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(root.join(STATE_DIR)).unwrap();

        assert_eq!(root_from(&nested), root);
    }

    #[test]
    fn open_at_reads_manifest_and_state() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(manifest_path(&root), "[[install]]\nskill = \"a\"\n").unwrap();
        state::save(
            &state_path(&root),
            &State {
                skills: vec![state::SkillState {
                    name: "a".to_owned(),
                    source_rel: "a".to_owned(),
                    target: ".agents/skills".to_owned(),
                    written_hash: "blake3:x".to_owned(),
                    source_hash: "blake3:y".to_owned(),
                    pinned: false,
                    installed_at: 1,
                    ..state::SkillState::default()
                }],
            },
        )
        .unwrap();

        let project = Project::open_at(&root).unwrap();
        assert_eq!(project.root(), root);
        assert!(project.manifest.install_for("a").is_some());
        assert!(
            !project.state.get("a", ".agents/skills").unwrap().pinned,
            "the fixture installs an unpinned copy"
        );
        assert_eq!(project.manifest_path(), manifest_path(&root));
        assert_eq!(project.state_path(), state_path(&root));
    }

    #[test]
    fn open_at_on_an_empty_directory_is_an_empty_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project = Project::open_at(tmp.path()).unwrap();

        assert_eq!(project.manifest, Manifest::default());
        assert_eq!(project.state, State::default());
        assert!(!project.manifest_path().exists());
    }

    #[test]
    fn document_reads_and_writes_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let project = Project::open_at(tmp.path()).unwrap();
        let mut document = project.document().unwrap();
        document["defaults"] = toml_edit::Item::Table(toml_edit::Table::new());

        project.write_document(&document).unwrap();

        assert_eq!(
            std::fs::read_to_string(project.manifest_path()).unwrap(),
            "[defaults]\n"
        );
    }
}
