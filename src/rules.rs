//! Rules and instruction files management (`skm rule`).
//!
//! Manages `AGENTS.md` and agent instruction files (e.g. `GEMINI.md`, `CLAUDE.md`)
//! across local projects and global agent configurations.
//!
//! Stores rules in the registered store as clean Markdown (without markers)
//! and injects/updates them in target destinations using non-destructive
//! comment blocks:
//!
//! ```markdown
//! <!-- BEGIN SKM INSTRUCTIONS -->
//! ... content from store ...
//! <!-- END SKM INSTRUCTIONS -->
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::hash;
use crate::project::Project;
use crate::registry::RuleState;
use crate::store;

pub const BEGIN_MARKER_BASE: &str = "<!-- BEGIN SKM INSTRUCTIONS";
pub const END_MARKER_BASE: &str = "<!-- END SKM INSTRUCTIONS";

/// An instruction rule available in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Identifier: "default" for AGENTS.md, or the file stem for rules/*.md.
    pub name: String,
    /// Absolute path to the rule file in the store.
    pub path: PathBuf,
    /// Path relative to the store root.
    pub rel_path: String,
    /// Whether this is the root AGENTS.md rule.
    pub is_default: bool,
}

/// Target flags for rule commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleTargetFlags {
    pub global: bool,
    pub agent: bool,
    pub antigravity: bool,
    pub claude: bool,
    pub opencode: bool,
    pub cursor: bool,
}

impl RuleTargetFlags {
    pub fn any_agent(self) -> bool {
        self.antigravity || self.claude || self.opencode || self.cursor || self.agent
    }
}

/// What action will be taken for a target file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    Created,
    Updated,
    Ok,
    Skipped,
    Removed,
}

impl RuleAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Ok => "ok",
            Self::Skipped => "skipped",
            Self::Removed => "removed",
        }
    }
}

/// One planned rule application step.
#[derive(Debug, Clone)]
pub struct RulePlan {
    pub rule_name: String,
    pub target_file: PathBuf,
    pub display_target: String,
    pub source_rel: String,
    pub action: RuleAction,
    pub new_content: Option<String>,
    pub store_hash: String,
    pub note: Option<String>,
}

/// Format the opening marker line.
pub fn format_begin_marker(tag: Option<&str>) -> String {
    match tag {
        Some(t) if !t.is_empty() && t != "default" && t != "AGENTS" => {
            format!("{BEGIN_MARKER_BASE}: {t} -->")
        }
        _ => format!("{BEGIN_MARKER_BASE} -->"),
    }
}

/// Format the closing marker line.
pub fn format_end_marker(tag: Option<&str>) -> String {
    match tag {
        Some(t) if !t.is_empty() && t != "default" && t != "AGENTS" => {
            format!("{END_MARKER_BASE}: {t} -->")
        }
        _ => format!("{END_MARKER_BASE} -->"),
    }
}

/// Wrap content inside standard comment markers.
pub fn wrap_in_markers(content: &str, tag: Option<&str>) -> String {
    let begin = format_begin_marker(tag);
    let end = format_end_marker(tag);
    let trimmed = content.trim_end();
    if trimmed.is_empty() {
        format!("{begin}\n{end}\n")
    } else {
        format!("{begin}\n{trimmed}\n{end}\n")
    }
}

/// Test if a line matches a BEGIN marker for the given tag.
pub fn is_matching_begin(line: &str, tag: Option<&str>) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with("<!--") || !trimmed.ends_with("-->") {
        return false;
    }
    let inner = trimmed
        .strip_prefix("<!--")
        .unwrap()
        .strip_suffix("-->")
        .unwrap()
        .trim();
    if let Some(rest) = inner.strip_prefix("BEGIN SKM INSTRUCTIONS") {
        let rest = rest.trim();
        match tag {
            None => rest.is_empty() || rest == ": default" || rest == ": AGENTS",
            Some(t) => {
                if t == "default" || t == "AGENTS" || t.is_empty() {
                    rest.is_empty() || rest == ": default" || rest == ": AGENTS"
                } else {
                    rest == format!(": {t}")
                }
            }
        }
    } else {
        false
    }
}

/// Test if a line matches an END marker for the given tag.
pub fn is_matching_end(line: &str, tag: Option<&str>) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with("<!--") || !trimmed.ends_with("-->") {
        return false;
    }
    let inner = trimmed
        .strip_prefix("<!--")
        .unwrap()
        .strip_suffix("-->")
        .unwrap()
        .trim();
    if let Some(rest) = inner.strip_prefix("END SKM INSTRUCTIONS") {
        let rest = rest.trim();
        match tag {
            None => rest.is_empty() || rest == ": default" || rest == ": AGENTS",
            Some(t) => {
                if t == "default" || t == "AGENTS" || t.is_empty() {
                    rest.is_empty() || rest == ": default" || rest == ": AGENTS"
                } else {
                    rest == format!(": {t}")
                }
            }
        }
    } else {
        false
    }
}

/// Non-destructively apply or update content in a marked block.
pub fn apply_block(
    existing_content: Option<&str>,
    new_content: &str,
    tag: Option<&str>,
    overwrite: bool,
) -> Result<String> {
    let Some(existing) = existing_content else {
        return Ok(wrap_in_markers(new_content, tag));
    };

    if existing.trim().is_empty() {
        return Ok(wrap_in_markers(new_content, tag));
    }

    let lines: Vec<&str> = existing.lines().collect();
    let mut begin_idx: Option<usize> = None;
    let mut end_idx: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        if is_matching_begin(line, tag) {
            if begin_idx.is_some() {
                bail!(
                    "duplicate BEGIN marker found in file; aborting to prevent ambiguous replacement"
                );
            }
            begin_idx = Some(i);
        } else if is_matching_end(line, tag) {
            if end_idx.is_some() {
                bail!(
                    "duplicate END marker found in file; aborting to prevent ambiguous replacement"
                );
            }
            end_idx = Some(i);
        }
    }

    match (begin_idx, end_idx) {
        (Some(b), Some(e)) => {
            if e < b {
                bail!(
                    "malformed instructions block: END marker appears before BEGIN marker; aborting to prevent data loss"
                );
            }
            let mut result = String::new();
            for line in &lines[..b] {
                result.push_str(line);
                result.push('\n');
            }
            result.push_str(&wrap_in_markers(new_content, tag));
            for (idx, line) in lines[e + 1..].iter().enumerate() {
                result.push_str(line);
                if idx + 1 < lines[e + 1..].len() || existing.ends_with('\n') {
                    result.push('\n');
                }
            }
            Ok(result)
        }
        (Some(_), None) => {
            bail!(
                "malformed instructions block: found BEGIN marker without matching END marker; aborting to prevent data loss"
            );
        }
        (None, Some(_)) => {
            bail!(
                "malformed instructions block: found END marker without matching BEGIN marker; aborting to prevent data loss"
            );
        }
        (None, None) => {
            if overwrite {
                Ok(wrap_in_markers(new_content, tag))
            } else {
                let mut result = existing.to_string();
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push('\n');
                result.push_str(&wrap_in_markers(new_content, tag));
                Ok(result)
            }
        }
    }
}

/// Extract inner content of a marked block.
pub fn extract_block(content: &str, tag: Option<&str>) -> Result<Option<String>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut begin_idx = None;
    let mut end_idx = None;

    for (i, line) in lines.iter().enumerate() {
        if is_matching_begin(line, tag) {
            begin_idx = Some(i);
        } else if is_matching_end(line, tag) {
            end_idx = Some(i);
        }
    }

    match (begin_idx, end_idx) {
        (Some(b), Some(e)) if b < e => {
            let inner = lines[b + 1..e].join("\n");
            Ok(Some(inner.trim().to_string()))
        }
        (Some(_), None) => {
            bail!("malformed instructions block: found BEGIN marker without matching END marker");
        }
        (None, Some(_)) => {
            bail!("malformed instructions block: found END marker without matching BEGIN marker");
        }
        _ => Ok(None),
    }
}

/// Remove a marked block, preserving content outside the markers.
pub fn remove_block(content: &str, tag: Option<&str>) -> Result<Option<String>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut begin_idx = None;
    let mut end_idx = None;

    for (i, line) in lines.iter().enumerate() {
        if is_matching_begin(line, tag) {
            begin_idx = Some(i);
        } else if is_matching_end(line, tag) {
            end_idx = Some(i);
        }
    }

    match (begin_idx, end_idx) {
        (Some(b), Some(e)) if b <= e => {
            let start_bound = b;
            let mut end_bound = e + 1;
            if start_bound > 0
                && lines[start_bound - 1].is_empty()
                && end_bound < lines.len()
                && lines[end_bound].is_empty()
            {
                end_bound += 1;
            }

            let mut result = String::new();
            for line in &lines[..start_bound] {
                result.push_str(line);
                result.push('\n');
            }
            for (idx, line) in lines[end_bound..].iter().enumerate() {
                result.push_str(line);
                if idx + 1 < lines[end_bound..].len() || content.ends_with('\n') {
                    result.push('\n');
                }
            }
            let trimmed = result.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(result))
            }
        }
        (Some(_), Some(_)) => {
            bail!(
                "malformed instructions block: END marker appears before BEGIN marker; aborting to prevent data loss"
            );
        }
        (Some(_), None) => {
            bail!(
                "malformed instructions block: found BEGIN marker without matching END marker; aborting to prevent data loss"
            );
        }
        (None, Some(_)) => {
            bail!(
                "malformed instructions block: found END marker without matching BEGIN marker; aborting to prevent data loss"
            );
        }
        (None, None) => Ok(Some(content.to_string())),
    }
}

/// List all rules available in the registered store root.
pub fn list_rules(store_root: &Path) -> Result<Vec<Rule>> {
    let mut rules = Vec::new();

    // Check for AGENTS.md or agents.md in store root
    let default_candidates = [store::AGENTS_FILE, "agents.md"];
    for candidate in default_candidates {
        let p = store_root.join(candidate);
        if p.is_file() {
            rules.push(Rule {
                name: "default".to_string(),
                path: p,
                rel_path: candidate.to_string(),
                is_default: true,
            });
            break;
        }
    }

    // Check for rules/ directory
    let rules_dir = store_root.join(store::RULES_SUBDIR);
    if let Ok(entries) = fs::read_dir(&rules_dir) {
        let mut sub_rules = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            let is_md = p.is_file() && p.extension() == Some(std::ffi::OsStr::new("md"));
            if let (true, Some(stem)) = (is_md, p.file_stem().and_then(|s| s.to_str())) {
                sub_rules.push(Rule {
                    name: stem.to_string(),
                    path: p.clone(),
                    rel_path: format!(
                        "{}/{}",
                        store::RULES_SUBDIR,
                        p.file_name().unwrap().to_string_lossy()
                    ),
                    is_default: false,
                });
            }
        }
        sub_rules.sort_by(|a, b| a.name.cmp(&b.name));
        rules.extend(sub_rules);
    }

    Ok(rules)
}

/// Find a specific rule by name in the store root.
pub fn find_rule(store_root: &Path, name: &str) -> Result<Rule> {
    let available = list_rules(store_root)?;
    let normalized = name.trim();

    if normalized.is_empty()
        || normalized == "default"
        || normalized == "AGENTS"
        || normalized == "agents"
    {
        if let Some(def) = available.iter().find(|r| r.is_default) {
            return Ok(def.clone());
        }
        // Fallback: check rules/default.md or rules/AGENTS.md
        if let Some(r) = available
            .iter()
            .find(|r| r.name == "default" || r.name == "AGENTS")
        {
            return Ok(r.clone());
        }
        bail!(
            "no default AGENTS.md found in the store root ({})",
            store_root.display()
        );
    }

    if let Some(r) = available.iter().find(|r| r.name == normalized) {
        return Ok(r.clone());
    }

    bail!("rule '{name}' not found in store; run `skm rule list` to see available rules");
}

/// Convert an absolute path to a portable display representation (using `~` for home).
pub fn display_target(path: &Path) -> String {
    let home = dirs::home_dir();
    match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.to_string_lossy()),
        None => path.to_string_lossy().into_owned(),
    }
}

/// Resolve target destination files for a rule command.
///
/// CRITICAL: In `--global` mode, NEVER fall back to `~/.agents/AGENTS.md`.
/// In non-interactive mode without flags, fail with a clear error.
pub fn resolve_rule_targets(
    _config: &Config,
    project_root: Option<&Path>,
    flags: RuleTargetFlags,
    file_override: Option<&str>,
    interactive: bool,
) -> Result<Vec<PathBuf>> {
    // 1. Explicit --file flag
    if let Some(file_str) = file_override {
        let trimmed = file_str.trim();
        if trimmed.is_empty() {
            bail!("--file must not be empty");
        }
        let expanded = store::expand_tilde(trimmed);
        let path = if expanded.is_absolute() {
            expanded
        } else if let Some(root) = project_root {
            root.join(expanded)
        } else {
            expanded
        };
        return Ok(vec![path]);
    }

    // 2. Global mode
    if flags.global {
        let mut targets = Vec::new();
        if flags.antigravity {
            targets.push(store::expand_tilde("~/.gemini/GEMINI.md"));
        }
        if flags.claude {
            targets.push(store::expand_tilde("~/.claude/CLAUDE.md"));
        }
        if flags.opencode {
            targets.push(store::expand_tilde("~/.config/opencode/AGENTS.md"));
        }
        if flags.cursor {
            targets.push(store::expand_tilde("~/.cursor/AGENTS.md"));
        }

        if !targets.is_empty() {
            return Ok(targets);
        }

        // No agent flags specified
        if interactive {
            return crate::interactive::select_rule_targets_global();
        }

        bail!(
            "--global requires specifying a target agent (--agy, --claude, --opencode, --cursor) or --file <path>"
        );
    }

    // 3. Project mode
    let root = match project_root {
        Some(r) => r.to_path_buf(),
        None => std::env::current_dir().context("cannot determine current directory")?,
    };

    if flags.any_agent() {
        let mut targets = Vec::new();
        if flags.antigravity {
            targets.push(root.join(".gemini").join("GEMINI.md"));
        }
        if flags.claude {
            targets.push(root.join("CLAUDE.md"));
        }
        if flags.opencode || flags.agent {
            targets.push(root.join(store::AGENTS_FILE));
        }
        if flags.cursor {
            targets.push(root.join(".cursor").join(store::AGENTS_FILE));
        }
        return Ok(targets);
    }

    // Default for project: AGENTS.md in project root
    Ok(vec![root.join(store::AGENTS_FILE)])
}

/// Normalize rule text by trimming leading/trailing whitespace for consistent hashing.
pub fn normalize_rule_content(content: &str) -> &str {
    content.trim()
}

/// Plan applying a rule to the resolved targets.
pub fn plan_apply(rule: &Rule, targets: &[PathBuf], overwrite: bool) -> Result<Vec<RulePlan>> {
    let rule_content = fs::read_to_string(&rule.path)
        .with_context(|| format!("reading store rule file {}", rule.path.display()))?;
    let store_hash = hash::content_hash(normalize_rule_content(&rule_content));
    let tag = if rule.is_default {
        None
    } else {
        Some(rule.name.as_str())
    };

    let mut plans = Vec::new();

    for target_file in targets {
        let display = display_target(target_file);
        let existing = match fs::read_to_string(target_file) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {}", target_file.display())),
        };

        let new_text = apply_block(existing.as_deref(), &rule_content, tag, overwrite)?;
        let action = if existing.is_none() {
            RuleAction::Created
        } else if let Ok(Some(current_block)) = extract_block(existing.as_deref().unwrap(), tag) {
            if hash::content_hash(&current_block) == store_hash {
                RuleAction::Ok
            } else {
                RuleAction::Updated
            }
        } else {
            RuleAction::Updated
        };

        plans.push(RulePlan {
            rule_name: rule.name.clone(),
            target_file: target_file.clone(),
            display_target: display,
            source_rel: rule.rel_path.clone(),
            action,
            new_content: Some(new_text),
            store_hash: store_hash.clone(),
            note: None,
        });
    }

    Ok(plans)
}

/// Apply the planned rule mutations and update the project registry.
pub fn apply_plans(project: &mut Project, plans: &[RulePlan], now: i64) -> Result<()> {
    for plan in plans {
        if let Some(content) = &plan.new_content {
            if let Some(parent) = plan.target_file.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating directory {}", parent.display()))?;
            }
            fs::write(&plan.target_file, content)
                .with_context(|| format!("writing {}", plan.target_file.display()))?;

            project.record.upsert_rule(RuleState {
                name: plan.rule_name.clone(),
                source_rel: plan.source_rel.clone(),
                target_file: plan.display_target.clone(),
                applied_hash: plan.store_hash.clone(),
                source_hash: plan.store_hash.clone(),
                applied_at: now,
            });
        }
    }
    project.save()
}

/// Plan removing an applied rule from the specified targets.
pub fn plan_remove(rule_name: &str, targets: &[PathBuf]) -> Result<Vec<RulePlan>> {
    let tag = if rule_name == "default" || rule_name == "AGENTS" {
        None
    } else {
        Some(rule_name)
    };

    let mut plans = Vec::new();

    for target_file in targets {
        let display = display_target(target_file);
        let existing = match fs::read_to_string(target_file) {
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                plans.push(RulePlan {
                    rule_name: rule_name.to_string(),
                    target_file: target_file.clone(),
                    display_target: display,
                    source_rel: String::new(),
                    action: RuleAction::Skipped,
                    new_content: None,
                    store_hash: String::new(),
                    note: Some("file does not exist".to_string()),
                });
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", target_file.display())),
        };

        let new_text = remove_block(&existing, tag)?;
        plans.push(RulePlan {
            rule_name: rule_name.to_string(),
            target_file: target_file.clone(),
            display_target: display,
            source_rel: String::new(),
            action: RuleAction::Removed,
            new_content: new_text,
            store_hash: String::new(),
            note: None,
        });
    }

    Ok(plans)
}

/// Execute rule removal plans and update the project registry.
pub fn apply_remove(project: &mut Project, plans: &[RulePlan]) -> Result<()> {
    for plan in plans {
        if plan.action == RuleAction::Removed {
            match &plan.new_content {
                Some(remaining) => {
                    fs::write(&plan.target_file, remaining)
                        .with_context(|| format!("updating {}", plan.target_file.display()))?;
                }
                None => {
                    // File is now empty of user content; remove file
                    let _ = fs::remove_file(&plan.target_file);
                }
            }
            project
                .record
                .remove_rule(&plan.rule_name, &plan.display_target);
        }
    }
    project.save()
}

/// Plan syncing all recorded rules in a project with the registered store.
pub fn plan_sync(config: &Config, project: &Project) -> Result<Vec<RulePlan>> {
    let store_path = config.store_path()?;
    let store_root = Path::new(&store_path);

    let mut plans = Vec::new();

    for recorded in &project.record.rules {
        let rule = find_rule(store_root, &recorded.name)?;
        let rule_content = fs::read_to_string(&rule.path)?;
        let store_hash = hash::content_hash(normalize_rule_content(&rule_content));
        let target_file = store::expand_tilde(&recorded.target_file);
        let tag = if rule.is_default {
            None
        } else {
            Some(rule.name.as_str())
        };

        let existing = match fs::read_to_string(&target_file) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {}", target_file.display())),
        };

        let action = if existing.is_none() {
            RuleAction::Created
        } else if let Ok(Some(current_block)) = extract_block(existing.as_deref().unwrap(), tag) {
            if hash::content_hash(&current_block) == store_hash {
                RuleAction::Ok
            } else {
                RuleAction::Updated
            }
        } else {
            RuleAction::Updated
        };

        let new_text = if action != RuleAction::Ok {
            Some(apply_block(existing.as_deref(), &rule_content, tag, false)?)
        } else {
            None
        };

        plans.push(RulePlan {
            rule_name: recorded.name.clone(),
            target_file,
            display_target: recorded.target_file.clone(),
            source_rel: rule.rel_path,
            action,
            new_content: new_text,
            store_hash,
            note: None,
        });
    }

    Ok(plans)
}

#[cfg(test)]
mod tests;
