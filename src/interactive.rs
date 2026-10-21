//! Interactive terminal multi-select picker for skills and packs.

use std::fmt;

use anyhow::{Result, bail};
use inquire::MultiSelect;

use crate::config::Config;
use crate::pack;
use crate::store;

/// A choice item displayed in the interactive multi-select prompt.
#[derive(Debug, Clone)]
pub struct Choice {
    pub name: String,
    pub description: Option<String>,
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(desc) = &self.description {
            write!(f, "{:<24} {}", self.name, desc)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

/// Prompt the user to interactively select skills from the store.
pub fn select_skills(config: &Config) -> Result<Vec<String>> {
    let report = store::scan(config.store_path()?)?;
    if !report.ready {
        bail!("{}", report.warnings.join("; "));
    }
    if report.skills.is_empty() {
        bail!("no skills found in the store; run `skm store scan` to inspect your store");
    }

    let choices: Vec<Choice> = report
        .skills
        .into_iter()
        .map(|s| Choice {
            name: s.name,
            description: s.description,
        })
        .collect();

    let prompt = MultiSelect::new("Select skills to install:", choices)
        .with_help_message("Press <space> to toggle, <a> to toggle all, <enter> to confirm");

    match prompt.prompt() {
        Ok(selected) => Ok(selected.into_iter().map(|c| c.name).collect()),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(Vec::new()),
        Err(err) => Err(anyhow::anyhow!("interactive prompt error: {err}")),
    }
}

/// Prompt the user to interactively select packs from the store.
pub fn select_packs(config: &Config) -> Result<Vec<String>> {
    let store_root = std::path::Path::new(config.store_path()?);
    let packs = pack::list(store_root)?;
    if packs.is_empty() {
        bail!("no packs found in the store; check <store>/packs/");
    }

    let choices: Vec<Choice> = packs
        .into_iter()
        .map(|p| Choice {
            name: p.name,
            description: p.description,
        })
        .collect();

    let prompt = MultiSelect::new("Select packs to install:", choices)
        .with_help_message("Press <space> to toggle, <a> to toggle all, <enter> to confirm");

    match prompt.prompt() {
        Ok(selected) => Ok(selected.into_iter().map(|c| c.name).collect()),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(Vec::new()),
        Err(err) => Err(anyhow::anyhow!("interactive prompt error: {err}")),
    }
}
