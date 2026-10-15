//! Command implementations.
//!
//! Commands never write to the user configuration on their own initiative:
//! `config show` prints defaults in memory when the file is missing, and only
//! `store add` / `store remove` touch the file (going through `toml_edit`, so
//! comments and formatting survive). Project commands work inside the project
//! found from the working directory: `add`, `remove` and `sync` write
//! `skills.toml` and `.skills-manager/state.toml`, `status` only reads.

use anyhow::{Result, bail};

use crate::cli::{Cli, Commands, ConfigCommand, StoreCommand};
use crate::config::{self, Config};
use crate::install::{self, Action, AddPlan, RemovePlan};
use crate::output::{BOLD, Printer, Style};
use crate::project::Project;
use crate::status::{self, Health};
use crate::store::{self, ScanReport};
use crate::sync::{self, SyncAction, SyncPlan};

/// Footer of a `--dry-run`, so piped output can never be mistaken for a real run.
const DRY_RUN_NOTE: &str = "Dry run: nothing was written.";

/// Width of the action keyword column of `skm sync` (the longest keyword).
const SYNC_KEYWORD_WIDTH: usize = 9;

/// Run the parsed CLI, flushing output before returning.
pub fn run(cli: Cli) -> Result<()> {
    let mut out = Printer::stdout();
    let outcome = dispatch(&mut out, cli);
    let flushed = out.flush();
    outcome?;
    flushed?;
    Ok(())
}

fn dispatch(out: &mut Printer, cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Config { action } => config_cmd(out, action),
        Commands::Store { action } => store_cmd(out, action),
        Commands::List => list_skills(out),
        Commands::Add {
            skills,
            agent,
            dir,
            pin,
            force,
            dry_run,
        } => add_skills(
            out,
            &skills,
            install::AddOptions {
                dir,
                agent,
                pin,
                force,
            },
            dry_run,
        ),
        Commands::Remove {
            skills,
            force,
            dry_run,
        } => remove_skills(out, &skills, install::RemoveOptions { force }, dry_run),
        Commands::Status => status_cmd(out),
        Commands::Sync { force, dry_run } => sync_cmd(out, force, dry_run),
        Commands::Projects => projects_cmd(out),
    }
}

/// `skm add`: plan every skill, then copy them and record them.
fn add_skills(
    out: &mut Printer,
    names: &[String],
    options: install::AddOptions,
    dry_run: bool,
) -> Result<()> {
    let config = config::load()?;
    let mut project = Project::open()?;
    let plans = install::plan_add(&config, &project, names, &options)?;

    if !dry_run {
        install::apply_add(&mut project, &plans)?;
    }

    let style = out.style();
    for plan in &plans {
        out.line(&add_line(style, plan, dry_run))?;
    }
    if dry_run && !plans.is_empty() {
        out.line(DRY_RUN_NOTE)?;
    }
    Ok(())
}

/// `Installed alpha -> .agents/skills/alpha`, with `(pinned)` when pinned.
fn add_line(style: Style, plan: &AddPlan, dry_run: bool) -> String {
    let verb = if dry_run {
        plan.action.dry_verb()
    } else {
        plan.action.verb()
    };
    let verb = match plan.action {
        Action::Replace => style.warn(verb),
        Action::Install | Action::Resync => style.ok(verb),
    };

    let mut line = format!("{verb} {} -> {}", style.ident(&plan.skill), plan.dest_rel);
    if plan.pinned {
        line.push_str(" (pinned)");
    }
    line
}

/// `skm remove`: plan every skill, then delete the copies and the entries.
fn remove_skills(
    out: &mut Printer,
    names: &[String],
    options: install::RemoveOptions,
    dry_run: bool,
) -> Result<()> {
    let config = config::load()?;
    let mut project = Project::open()?;
    let plans = install::plan_remove(&config, &project, names, &options)?;

    if !dry_run {
        install::apply_remove(&mut project, &plans)?;
    }

    let style = out.style();
    for plan in &plans {
        out.line(&remove_line(style, plan, dry_run))?;
    }
    if dry_run && !plans.is_empty() {
        out.line(DRY_RUN_NOTE)?;
    }
    Ok(())
}

/// `Removed alpha (.agents/skills/alpha)`.
fn remove_line(style: Style, plan: &RemovePlan, dry_run: bool) -> String {
    let verb = if dry_run {
        plan.action.dry_verb()
    } else {
        plan.action.verb()
    };
    let detail = if plan.copies.is_empty() {
        "was never installed".to_owned()
    } else {
        let copies: Vec<&str> = plan
            .copies
            .iter()
            .map(|copy| copy.dest_rel.as_str())
            .collect();
        copies.join(", ")
    };
    format!(
        "{} {} ({detail})",
        style.warn(verb),
        style.ident(&plan.skill)
    )
}

/// `skm sync`: reconcile every declared copy, then report the plan.
fn sync_cmd(out: &mut Printer, force: bool, dry_run: bool) -> Result<()> {
    let config = config::load()?;
    let mut project = Project::open()?;
    let plans = sync::plan_sync(&config, &project, force)?;

    if !dry_run {
        sync::apply_sync(&mut project, &plans)?;
    }

    if plans.is_empty() {
        out.line("Nothing to sync: no skills registered in this project and nothing found in the target directories.")?;
        return Ok(());
    }

    let style = out.style();
    for plan in &plans {
        out.line(&sync_line(style, plan))?;
    }
    out.line(&sync_summary(&plans))?;
    if dry_run {
        out.line(DRY_RUN_NOTE)?;
    }
    Ok(())
}

/// `detached  draft -> .agents/skills/draft (local modifications)`.
fn sync_line(style: Style, plan: &SyncPlan) -> String {
    let keyword = format!(
        "{:<width$}",
        plan.action.keyword(),
        width = SYNC_KEYWORD_WIDTH
    );
    let keyword = match plan.action {
        SyncAction::Ok
        | SyncAction::Updated
        | SyncAction::Installed
        | SyncAction::Restored
        | SyncAction::Adopted => style.ok(&keyword),
        SyncAction::Pinned => style.ident(&keyword),
        SyncAction::Replaced | SyncAction::Detached | SyncAction::Skipped => style.warn(&keyword),
        SyncAction::Unmanaged => style.fail(&keyword),
    };

    let mut line = format!(
        "{keyword} {} -> {}",
        style.ident(&plan.skill),
        plan.dest_rel
    );
    if let Some(note) = plan.note() {
        line.push_str(&format!(" ({note})"));
    }
    line
}

/// `3 entries: 1 updated, 1 detached, 1 ok`.
fn sync_summary(plans: &[SyncPlan]) -> String {
    let mut counts = Vec::new();
    for action in SyncAction::ALL {
        let count = plans.iter().filter(|plan| plan.action == action).count();
        if count > 0 {
            counts.push(format!("{count} {}", action.keyword()));
        }
    }
    let noun = if plans.len() == 1 { "entry" } else { "entries" };
    format!("{} {noun}: {}", plans.len(), counts.join(", "))
}

/// `skm status`: read only summary of every declared and unmanaged skill.
fn status_cmd(out: &mut Printer) -> Result<()> {
    let config = config::load()?;
    let project = Project::open()?;
    let found = status::rows(&config, &project)?;
    if found.is_empty() {
        out.line("Nothing to report: no skills registered in this project and nothing found in the target directories.")?;
        return Ok(());
    }

    let style = out.style();
    let rows: Vec<Vec<String>> = found
        .iter()
        .map(|row| {
            vec![
                style.ident(&row.skill),
                row.target.clone(),
                health_cell(style, row.health),
            ]
        })
        .collect();
    out.table(&["SKILL", "TARGET", "STATE"], &rows)?;
    Ok(())
}

/// Colored STATE cell of `skm status`, with its note when it has one.
fn health_cell(style: Style, health: Health) -> String {
    let label = match health {
        Health::Ok => style.ok(health.label()),
        Health::Pinned | Health::Detached | Health::Skipped => style.ident(health.label()),
        Health::Modified | Health::Behind => style.warn(health.label()),
        Health::Missing | Health::Unmanaged => style.fail(health.label()),
    };
    match health.note() {
        Some(note) => format!("{label} ({note})"),
        None => label,
    }
}

fn config_cmd(out: &mut Printer, action: ConfigCommand) -> Result<()> {
    let path = config::path()?;
    match action {
        ConfigCommand::Path => out.line(&path.display().to_string())?,
        ConfigCommand::Show => match config::read_raw(&path)? {
            Some(text) => out.line(text.trim_end())?,
            None => {
                let defaults = toml::to_string_pretty(&Config::default())?;
                out.line(defaults.trim_end())?;
            }
        },
    }
    Ok(())
}

fn store_cmd(out: &mut Printer, action: Option<StoreCommand>) -> Result<()> {
    // Plain `skm store` shows the registered store, exactly like `store show`.
    match action.unwrap_or(StoreCommand::Show) {
        StoreCommand::Add { path, create } => store_add(out, &path, create),
        StoreCommand::Show => store_show(out),
        StoreCommand::Remove => store_remove(out),
        StoreCommand::Scan => store_scan(out),
    }
}

fn store_add(out: &mut Printer, path: &str, create: bool) -> Result<()> {
    let outcome = store::add(path, create)?;
    let style = out.style();
    if outcome.unchanged {
        out.line(&format!(
            "{}: {}",
            style.ident("store already registered"),
            outcome.path
        ))?;
        return Ok(());
    }

    let mut line = format!("{} = {}", style.ident("store"), outcome.path);
    if outcome.path != outcome.resolved.to_string_lossy() {
        line.push_str(&format!(" -> {}", outcome.resolved.display()));
    }
    if let Some(previous) = &outcome.replaced {
        line.push_str(&format!(" (was {previous})"));
    }
    out.line(&line)?;
    Ok(())
}

fn store_remove(out: &mut Printer) -> Result<()> {
    let path = store::remove()?;
    let style = out.style();
    out.line(&format!(
        "{} store {}",
        style.warn("Removed"),
        style.ident(&path)
    ))?;
    Ok(())
}

/// `skm store show` and plain `skm store`: the registered store at a glance.
fn store_show(out: &mut Printer) -> Result<()> {
    let config = config::load()?;
    let report = store::scan(config.store_path()?)?;
    let style = out.style();
    out.line(&style.paint(BOLD, "Store"))?;
    out.line(&format!("  {:<9} {}", "path", path_label(&report)))?;
    out.line(&format!(
        "  {:<9} {}",
        "state",
        state_label(style, report.ready)
    ))?;
    out.line(&format!("  {:<9} {}", "layout", layout_label(&report)))?;
    out.line(&format!("  {:<9} {}", "skills", report.skills.len()))?;
    Ok(())
}

fn store_scan(out: &mut Printer) -> Result<()> {
    let config = config::load()?;
    render_report(out, &store::scan(config.store_path()?)?)
}

fn render_report(out: &mut Printer, report: &ScanReport) -> Result<()> {
    let style = out.style();
    out.line(&style.paint(BOLD, "Store"))?;
    out.line(&format!("  {:<9} {}", "path", path_label(report)))?;
    out.line(&format!(
        "  {:<9} {}",
        "state",
        state_label(style, report.ready)
    ))?;
    out.line(&format!("  {:<9} {}", "layout", layout_label(report)))?;
    out.line(&format!("  {:<9} {}", "skills", report.skills.len()))?;
    for skill in &report.skills {
        out.line(&format!(
            "    - {} ({})",
            style.ident(&skill.name),
            skill.rel_path
        ))?;
    }
    out.line(&format!("  {:<9} {}", "packs", report.packs))?;
    if !report.warnings.is_empty() {
        out.line(&format!("  {:<9} {}", "warnings", report.warnings.len()))?;
        for warning in &report.warnings {
            out.line(&format!("    {} {warning}", style.warn("!")))?;
        }
    }
    Ok(())
}

fn list_skills(out: &mut Printer) -> Result<()> {
    let config = config::load()?;
    let report = store::scan(config.store_path()?)?;
    if !report.ready {
        // A registered store that cannot be read is an error, not an empty list.
        bail!("{}", report.warnings.join("; "));
    }

    let style = out.style();
    if report.skills.is_empty() {
        out.line("No skills found in the store.")?;
        return Ok(());
    }

    let rows: Vec<Vec<String>> = report
        .skills
        .iter()
        .map(|skill| vec![style.ident(&skill.name), skill.rel_path.clone()])
        .collect();
    out.table(&["SKILL", "PATH"], &rows)?;
    Ok(())
}

/// `OK` / `MISSING` marker for a store or report.
fn state_label(style: Style, ready: bool) -> String {
    if ready {
        style.ok("OK")
    } else {
        style.fail("MISSING")
    }
}

fn layout_label(report: &ScanReport) -> String {
    report
        .layout
        .map_or_else(|| "-".to_owned(), |layout| layout.as_str().to_owned())
}

/// `path written by the user`, plus the expanded path when they differ.
fn path_label(report: &ScanReport) -> String {
    if report.path_as_written == report.resolved.to_string_lossy() {
        report.path_as_written.clone()
    } else {
        format!(
            "{} -> {}",
            report.path_as_written,
            report.resolved.display()
        )
    }
}

fn projects_cmd(out: &mut Printer) -> Result<()> {
    let registry = crate::registry::Registry::load()?;
    if registry.projects.is_empty() {
        out.line("No projects registered.")?;
        return Ok(());
    }
    let style = out.style();
    let mut rows = Vec::new();
    for (key, record) in &registry.projects {
        let path = crate::registry::resolve_project_key(key);
        let exists = path.is_dir();
        let count = record.skills.len();
        let status = if exists {
            style.ok("active")
        } else {
            style.warn("missing on disk")
        };
        rows.push(vec![
            key.clone(),
            format!("{count} {}", if count == 1 { "skill" } else { "skills" }),
            status,
        ]);
    }
    out.table(&["PROJECT", "INSTALLED", "STATUS"], &rows)?;
    Ok(())
}
