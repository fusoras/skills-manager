//! Project layout: finding the project root and managing its record in the central registry.
//!
//! A project is completely clean: no `skills.toml` and no `.skills-manager/`
//! directories exist inside it. Its root is determined by finding the nearest
//! ancestor holding `.git` or recorded in the global project registry.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::registry::{ProjectRecord, Registry};

/// Project root of the current working directory.
pub fn root() -> Result<PathBuf> {
    let start = std::env::current_dir().context("cannot determine the working directory")?;
    Ok(root_from(&start))
}

/// [`root`] for an explicit starting directory.
pub fn root_from(start: &Path) -> PathBuf {
    let registry = Registry::load().unwrap_or_default();
    for candidate in start.ancestors() {
        if candidate.join(".git").exists() || registry.get_project(candidate).is_some() {
            return candidate.to_path_buf();
        }
    }
    start.to_path_buf()
}

/// An open project: its root directory, registry path, and its record.
pub struct Project {
    root: PathBuf,
    registry_path: PathBuf,
    /// Configuration and installed skills from the central registry.
    pub record: ProjectRecord,
}

impl Project {
    /// Open the project containing the working directory.
    pub fn open() -> Result<Self> {
        let root = root()?;
        Self::open_at(&root)
    }

    /// [`Project::open`] for an explicit root.
    pub fn open_at(root: &Path) -> Result<Self> {
        Self::open_at_with(root, &crate::registry::path()?)
    }

    /// Open a project with an explicit registry path (used in tests and isolation).
    pub fn open_at_with(root: &Path, registry_path: &Path) -> Result<Self> {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let registry = Registry::load_from(registry_path)?;
        let record = registry
            .get_project(&canonical)
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            root: canonical,
            registry_path: registry_path.to_path_buf(),
            record,
        })
    }

    /// Project root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Persist changes to the central project registry.
    ///
    /// Leaves the project directory completely clean: zero metadata files.
    pub fn save(&self) -> Result<()> {
        let mut registry = Registry::load_from(&self.registry_path)?;
        if self.record.is_empty() {
            registry.remove_project(&self.root);
        } else {
            *registry.ensure_project(&self.root) = self.record.clone();
        }
        registry.save_to(&self.registry_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn root_from_walks_up_to_git() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let nested = root.join("src").join("deep");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();

        assert_eq!(root_from(&nested), root);
        assert_eq!(root_from(&root), root);
        assert_eq!(root_from(tmp.path()), tmp.path());
    }

    #[test]
    fn open_at_on_empty_directory_is_empty_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let registry_path = tmp.path().join("projects.toml");
        let project = Project::open_at_with(tmp.path(), &registry_path).unwrap();

        assert_eq!(project.record, ProjectRecord::default());
        // Verify no files or directories were created in the project
        let entries: Vec<_> = fs::read_dir(tmp.path()).unwrap().collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn save_updates_central_registry_without_dirtying_project() {
        let tmp = tempfile::tempdir().unwrap();
        let registry_path = tmp.path().join("state").join("projects.toml");
        let project_dir = tmp.path().join("my-project");
        fs::create_dir_all(&project_dir).unwrap();

        let mut project = Project::open_at_with(&project_dir, &registry_path).unwrap();
        project.record.target = Some(".agents/skills".to_owned());
        project.save().unwrap();

        // Project directory must contain NO files
        let entries: Vec<_> = fs::read_dir(&project_dir).unwrap().collect();
        assert!(
            entries.is_empty(),
            "Project directory must remain 100% clean!"
        );

        // Central registry must exist and contain the project
        let registry = Registry::load_from(&registry_path).unwrap();
        assert!(registry.get_project(&project_dir).is_some());
    }
}
