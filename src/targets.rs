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

/// Targets a declared skill may live in: what the state recorded, else the default.
pub fn state_targets(project: &Project, name: &str, default_target: &str) -> Vec<String> {
    let mut targets: Vec<String> = project
        .state
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
        .state
        .targets()
        .iter()
        .map(|target| (*target).to_owned())
        .collect();
    targets.push(default_target.to_owned());
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
    use crate::state::{self, SkillState, State};

    /// An empty project with `entries` written to its state file.
    fn project_with(entries: Vec<SkillState>) -> (tempfile::TempDir, Project) {
        let tmp = tempfile::tempdir().unwrap();
        if !entries.is_empty() {
            state::save(
                &crate::project::state_path(tmp.path()),
                &State { skills: entries },
            )
            .unwrap();
        }
        let project = Project::open_at(tmp.path()).unwrap();
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
            entry("alpha", "custom/skills"),
            entry("alpha", "custom/skills"),
            entry("beta", "other-target"),
        ]);

        assert_eq!(
            state_targets(&project, "alpha", ".agents/skills"),
            ["custom/skills"]
        );
        assert_eq!(
            state_targets(&project, "beta", ".agents/skills"),
            ["other-target"]
        );
    }

    #[test]
    fn known_targets_unions_the_state_and_the_default() {
        let (_tmp, project) = project_with(vec![entry("alpha", "custom/skills")]);

        assert_eq!(
            known_targets(&project, ".agents/skills"),
            [".agents/skills", "custom/skills"]
        );
    }

    #[test]
    fn subdirectories_is_sorted_and_skips_hidden_entries_and_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("zulu")).unwrap();
        fs::create_dir_all(tmp.path().join("alpha")).unwrap();
        fs::create_dir_all(tmp.path().join(".hidden")).unwrap();
        fs::write(tmp.path().join("loose.md"), "").unwrap();

        assert_eq!(subdirectories(tmp.path()).unwrap(), ["alpha", "zulu"]);
    }

    #[test]
    fn store_root_follows_the_registered_config_path() {
        let config = Config {
            store: Some("~/folder-skills".to_owned()),
            ..Config::default()
        };
        assert_eq!(
            store_root(&config),
            Some(store::expand_tilde("~/folder-skills"))
        );
        assert_eq!(store_root(&Config::default()), None);
    }
}
