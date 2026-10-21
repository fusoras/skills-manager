//! Target directories of a project: where copies live and which ones to scan.
//!
//! `status` and `sync` need the same answers (which directories to look at and
//! where the store is rooted), so they share them here instead of drifting apart.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::project::Project;
use crate::store;

/// Targets a skill may live in: what the project record saved, else the default.
pub fn state_targets(project: &Project, name: &str, default_target: &str) -> Vec<String> {
    let mut targets: Vec<String> = project
        .record
        .skills
        .iter()
        .filter(|entry| entry.name == name)
        .map(|entry| entry.target.clone())
        .collect();
    if targets.is_empty() {
        targets.push(default_target.to_owned());
    }
    targets.sort();
    targets.dedup();
    targets
}

/// Every target worth looking at for unmanaged directories.
pub fn known_targets(project: &Project, default_target: &str) -> Vec<String> {
    let mut targets: Vec<String> = project
        .record
        .targets()
        .iter()
        .map(|target| (*target).to_owned())
        .collect();
    if !project.is_global {
        targets.push(default_target.to_owned());
    }
    targets.sort();
    targets.dedup();
    targets
}

/// Names of the visible subdirectories of `dir`, sorted.
pub fn subdirectories(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading {}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // Hidden entries are never skills (the store scan keeps the same rule).
        if name.starts_with('.') || !entry.path().is_dir() {
            continue;
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

/// Root of the global store, expanded; `None` when none is registered.
pub fn store_root(config: &Config) -> Option<PathBuf> {
    config.store.as_deref().map(store::expand_tilde)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::SkillState;

    fn project_with(entries: Vec<SkillState>) -> (tempfile::TempDir, Project) {
        let tmp = tempfile::tempdir().unwrap();
        let mut project = Project::open_at(tmp.path()).unwrap();
        for entry in entries {
            project.record.upsert(entry);
        }
        (tmp, project)
    }

    fn entry(name: &str, target: &str) -> SkillState {
        SkillState {
            name: name.to_owned(),
            target: target.to_owned(),
            ..SkillState::default()
        }
    }

    #[test]
    fn state_targets_falls_back_to_the_default() {
        let (_tmp, project) = project_with(Vec::new());

        assert_eq!(
            state_targets(&project, "alpha", ".agents/skills"),
            [".agents/skills"]
        );
    }

    #[test]
    fn state_targets_uses_the_recorded_targets_of_that_skill_only() {
        let (_tmp, project) = project_with(vec![
            entry("alpha", "custom/one"),
            entry("alpha", "custom/two"),
            entry("beta", "custom/three"),
        ]);

        assert_eq!(
            state_targets(&project, "alpha", ".agents/skills"),
            ["custom/one", "custom/two"]
        );
    }

    #[test]
    fn known_targets_unions_the_state_and_the_default() {
        let (_tmp, project) = project_with(vec![
            entry("alpha", "custom/one"),
            entry("beta", "custom/two"),
        ]);

        assert_eq!(
            known_targets(&project, ".agents/skills"),
            [".agents/skills", "custom/one", "custom/two"]
        );
    }

    #[test]
    fn subdirectories_is_sorted_and_skips_hidden_entries_and_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("zebra")).unwrap();
        fs::create_dir_all(dir.join("alpha")).unwrap();
        fs::create_dir_all(dir.join(".hidden")).unwrap();
        fs::write(dir.join("file.txt"), "hello").unwrap();

        assert_eq!(
            subdirectories(dir).unwrap(),
            vec!["alpha".to_owned(), "zebra".to_owned()]
        );
    }

    #[test]
    fn store_root_follows_the_registered_config_path() {
        let config = Config {
            store: Some("/tmp/store".to_owned()),
            ..Config::default()
        };
        assert_eq!(store_root(&config), Some(PathBuf::from("/tmp/store")));

        let empty = Config::default();
        assert_eq!(store_root(&empty), None);
    }
}
