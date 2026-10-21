//! Interactive terminal multi-select picker for skills and packs.

use anyhow::{Result, bail};
use inquire::MultiSelect;
use inquire::ui::{Color, RenderConfig, StyleSheet};

use crate::config::Config;
use crate::pack;
use crate::store;

/// Gray footer help message separated from option list by an empty line spacer.
const HELP_MESSAGE: &str =
    "\r\x1b[K\n[Press <space> to toggle, <a> to toggle all, <enter> to confirm";

fn render_config() -> RenderConfig<'static> {
    RenderConfig::default_colored().with_help_message(StyleSheet::empty().with_fg(Color::DarkGrey))
}

/// Calculate page size so all options display without scrolling unless they
/// overflow the available terminal height.
fn page_size_for(count: usize) -> usize {
    page_size_from(count, crate::output::terminal_height())
}

fn page_size_from(count: usize, term_height: Option<usize>) -> usize {
    // Reserve lines for:
    // - Prompt header: "? Select skills to install:" (1 line)
    // - Empty spacer line (1 line)
    // - Footer help line: "[Press <space>...]" (1 line)
    // - Terminal breathing margin (2 lines)
    const RESERVED_LINES: usize = 5;

    if let Some(height) = term_height {
        let available = height.saturating_sub(RESERVED_LINES);
        if count <= available {
            count.max(1)
        } else {
            available.max(3)
        }
    } else {
        count.max(1)
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

    let skills: Vec<String> = report.skills.into_iter().map(|s| s.name).collect();
    let count = skills.len();

    let prompt = MultiSelect::new("Select skills to install:", skills)
        .with_render_config(render_config())
        .with_page_size(page_size_for(count))
        .with_help_message(HELP_MESSAGE);

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
    let count = packs.len();

    let prompt = MultiSelect::new("Select packs to install:", packs)
        .with_render_config(render_config())
        .with_page_size(page_size_for(count))
        .with_help_message(HELP_MESSAGE);

    match prompt.prompt() {
        Ok(selected) => Ok(selected),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(Vec::new()),
        Err(err) => Err(anyhow::anyhow!("interactive prompt error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_shows_all_when_fitting_terminal() {
        // 10 items fit comfortably in a 40-line terminal (35 available lines)
        assert_eq!(page_size_from(10, Some(40)), 10);
        assert_eq!(page_size_from(25, Some(40)), 25);
        assert_eq!(page_size_from(35, Some(40)), 35);
    }

    #[test]
    fn page_size_scrolls_only_when_overflowing_terminal() {
        // 50 items overflow a 40-line terminal (35 available lines -> page size 35)
        assert_eq!(page_size_from(50, Some(40)), 35);
        // Small terminal (7 lines -> 2 available, clamped to minimum 3)
        assert_eq!(page_size_from(10, Some(7)), 3);
    }

    #[test]
    fn page_size_falls_back_when_terminal_height_unknown() {
        assert_eq!(page_size_from(12, None), 12);
        assert_eq!(page_size_from(0, None), 1);
    }
}
