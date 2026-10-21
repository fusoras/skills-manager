//! Interactive terminal multi-select picker for skills and packs.

use anyhow::{Result, bail};
use inquire::MultiSelect;
use inquire::ui::{Color, RenderConfig, StyleSheet};

use crate::config::Config;
use crate::pack;
use crate::store;

fn render_config() -> RenderConfig<'static> {
    RenderConfig::default_colored().with_help_message(StyleSheet::empty().with_fg(Color::DarkGrey))
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

    let skills: Vec<String> = report.skills.into_iter().map(|s| s.name).collect();

    let prompt = MultiSelect::new("Select skills to install:", skills)
        .with_render_config(render_config())
        .with_help_message("Press <space> to toggle, <a> to toggle all, <enter> to confirm");

    match prompt.prompt() {
        Ok(selected) => Ok(selected),
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

    let packs: Vec<String> = packs.into_iter().map(|p| p.name).collect();

    let prompt = MultiSelect::new("Select packs to install:", packs)
        .with_render_config(render_config())
        .with_help_message("Press <space> to toggle, <a> to toggle all, <enter> to confirm");

    match prompt.prompt() {
        Ok(selected) => Ok(selected),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(Vec::new()),
        Err(err) => Err(anyhow::anyhow!("interactive prompt error: {err}")),
    }
}
