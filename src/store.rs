//! The global store: registration, layout detection and scanning.
//!
//! The store is a single directory holding skills, registered by path (never by
//! name) in the global configuration. Two layouts are supported:
//!
//! * `catalog`: the store contains a `skills/` directory; skills are its subdirectories.
//! * `flat`: skills are direct subdirectories of the store root (hidden entries and
//!   `packs/` are ignored).
//!
//! The path is normalized when registered: a directory inside the user's home is
//! stored as `~/...` (readable and portable), anything else as an absolute path.
//! `~` is expanded whenever the store is used.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::{self, NO_STORE};
use crate::skill::{self, Skill};

/// Directory that turns a store into `catalog` layout when present.
pub const SKILLS_SUBDIR: &str = "skills";

/// Directory holding pack definitions; it is never treated as a skill.
pub const PACKS_SUBDIR: &str = "packs";

/// Layout of a store on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Skills live directly under the store root.
    Flat,
    /// Skills live under `<store>/skills/`.
    Catalog,
}

impl Layout {
    /// Stable lowercase identifier used in output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Catalog => "catalog",
        }
    }
}

/// Expand a leading `~` / `~/` using the current user's home directory.
///
/// `~user/...` forms are not supported and are returned untouched.
pub fn expand_tilde(input: &str) -> PathBuf {
    expand_tilde_with(dirs::home_dir().as_deref(), input)
}

/// [`expand_tilde`] with an explicit home directory, for callers and tests.
pub fn expand_tilde_with(home: Option<&Path>, input: &str) -> PathBuf {
    if input == "~" {
        return home.map_or_else(|| PathBuf::from(input), Path::to_path_buf);
    }
    if let Some(rest) = input.strip_prefix("~/")
        && let Some(home) = home
    {
        return home.join(rest);
    }
    PathBuf::from(input)
}

/// Normalize a user-supplied store path into the portable form persisted in the
/// configuration.
///
/// The path is made absolute (`~` expanded, relative paths resolved against the
/// current directory) and then, when it lies inside the user's home, rewritten
/// as `~/...`; anything else is kept absolute. `~` is expanded again whenever
/// the store is used, so the stored value stays correct from any working
/// directory and on any machine.
pub fn normalize_store_path(input: &str) -> Result<String> {
    let cwd = env::current_dir().context("cannot determine the current directory")?;
    Ok(normalize_store_path_with(
        dirs::home_dir().as_deref(),
        &cwd,
        input,
    ))
}

/// [`normalize_store_path`] with an explicit home and working directory, for
/// callers and tests.
pub fn normalize_store_path_with(home: Option<&Path>, cwd: &Path, input: &str) -> String {
    let expanded = expand_tilde_with(home, input);
    let absolute = lexical_normalize(&if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    });

    match home.and_then(|home| absolute.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.to_string_lossy()),
        None => absolute.to_string_lossy().into_owned(),
    }
}

/// Resolve `.` and `..` without touching the filesystem, so it also works for
/// paths that do not exist yet.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Layout of a store root: `catalog` when `<root>/skills/` is a directory.
pub fn detect_layout(root: &Path) -> Layout {
    if root.join(SKILLS_SUBDIR).is_dir() {
        Layout::Catalog
    } else {
        Layout::Flat
    }
}

/// Directory that directly contains the skill directories.
pub fn skills_root(root: &Path, layout: Layout) -> PathBuf {
    match layout {
        Layout::Catalog => root.join(SKILLS_SUBDIR),
        Layout::Flat => root.to_path_buf(),
    }
}

/// Result of scanning the store.
#[derive(Debug, Clone)]
pub struct ScanReport {
    /// Path exactly as registered.
    pub path_as_written: String,
    /// Tilde-expanded path.
    pub resolved: PathBuf,
    /// The store root exists and is a directory.
    pub ready: bool,
    /// Detected layout; `None` when the root is not readable.
    pub layout: Option<Layout>,
    /// Skills found, sorted by name.
    pub skills: Vec<Skill>,
    /// Number of `*.toml` pack definitions in `<root>/packs/`.
    pub packs: usize,
    /// Non-fatal findings, sorted for stable output.
    pub warnings: Vec<String>,
}

/// Outcome of `skm store add`.
#[derive(Debug, Clone)]
pub struct AddOutcome {
    /// Normalized path persisted in the configuration (`~/...` under the home,
    /// absolute otherwise).
    pub path: String,
    /// Tilde-expanded, absolute path.
    pub resolved: PathBuf,
    /// The same path was already registered: nothing changed.
    pub unchanged: bool,
    /// Path registered before, when this run replaced another store.
    pub replaced: Option<String>,
}

/// Scan the store registered as `path_as_written`.
pub fn scan(path_as_written: &str) -> Result<ScanReport> {
    let resolved = expand_tilde(path_as_written);
    let mut warnings = Vec::new();

    if !resolved.is_dir() {
        warnings.push(if resolved.exists() {
            format!("path is not a directory: {}", resolved.display())
        } else {
            format!("path does not exist: {}", resolved.display())
        });
        return Ok(ScanReport {
            path_as_written: path_as_written.to_owned(),
            resolved,
            ready: false,
            layout: None,
            skills: Vec::new(),
            packs: 0,
            warnings,
        });
    }

    let layout = detect_layout(&resolved);
    let (skills, mut scan_warnings, packs) = scan_root(&resolved, layout)?;
    warnings.append(&mut scan_warnings);
    Ok(ScanReport {
        path_as_written: path_as_written.to_owned(),
        resolved,
        ready: true,
        layout: Some(layout),
        skills,
        packs,
        warnings,
    })
}

/// Register the global store, creating its directory when `create` is set.
///
/// The user's path is normalized before it is persisted: made absolute (`~`
/// expanded, relative resolved against the current directory) and rewritten as
/// `~/...` when inside the home, otherwise kept absolute. Registering the path
/// that is already registered changes nothing; any other path replaces the
/// previous store.
pub fn add(path_as_written: &str, create: bool) -> Result<AddOutcome> {
    if path_as_written.trim().is_empty() {
        bail!("store path must not be empty");
    }

    let stored = normalize_store_path(path_as_written)?;
    let resolved = expand_tilde(&stored);
    let current = config::load()?;
    let registered = current
        .store
        .as_deref()
        .map(|path| (path.to_owned(), expand_tilde(path)));

    // Validate the path before touching the file: a rejected path must not
    // leave a newly created directory or a rewritten config behind.
    ensure_directory(&stored, &resolved, create)?;

    if let Some((existing, existing_resolved)) = &registered
        && (existing == &stored || *existing_resolved == resolved)
    {
        return Ok(AddOutcome {
            path: stored,
            resolved,
            unchanged: true,
            replaced: None,
        });
    }

    config::update(|document| {
        config::doc_set_store(document, &stored);
        Ok(())
    })?;
    Ok(AddOutcome {
        path: stored,
        resolved,
        unchanged: false,
        replaced: registered.map(|(existing, _)| existing),
    })
}

/// Unregister the global store, returning its path as written.
pub fn remove() -> Result<String> {
    let current = config::load()?;
    let Some(path) = current.store.clone() else {
        bail!("{NO_STORE}");
    };
    config::update(|document| {
        if !config::doc_remove_store(document) {
            bail!("{NO_STORE}");
        }
        Ok(())
    })?;
    Ok(path)
}

/// Ensure the store path is an existing directory, creating it on `--create`.
fn ensure_directory(label: &str, resolved: &Path, create: bool) -> Result<()> {
    if !resolved.exists() {
        if !create {
            bail!("path does not exist: {label} (pass --create to create it)");
        }
        fs::create_dir_all(resolved).with_context(|| format!("creating {}", resolved.display()))?;
    } else if !resolved.is_dir() {
        bail!("path is not a directory: {label}");
    }
    Ok(())
}

/// Find skill directories under `root` and collect skills, warnings and pack count.
fn scan_root(root: &Path, layout: Layout) -> Result<(Vec<Skill>, Vec<String>, usize)> {
    let base = skills_root(root, layout);
    let mut skills = Vec::new();
    let mut warnings = Vec::new();

    for dir in subdirectories(&base)? {
        if is_hidden(&dir) || (layout == Layout::Flat && is_packs_dir(&dir)) {
            continue;
        }
        if skill::is_skill_dir(&dir) {
            let found = skill::load(&dir, root)?;
            if found.named && found.name != found.dir_name {
                warnings.push(format!(
                    "{}: frontmatter name '{}' does not match directory name '{}'",
                    found.rel_path, found.name, found.dir_name
                ));
            }
            skills.push(found);
        } else {
            warnings.push(format!(
                "{}: directory without {}",
                skill::relative_path(&dir, root),
                skill::SKILL_FILE
            ));
        }
    }

    skills.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.rel_path.cmp(&b.rel_path))
    });
    Ok((skills, warnings, count_packs(root)?))
}

/// Sorted subdirectories of `dir`; empty when `dir` does not exist.
fn subdirectories(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", dir.display())),
    };

    let mut directories = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        if path.is_dir() {
            directories.push(path);
        }
    }
    directories.sort();
    Ok(directories)
}

/// Number of `*.toml` files in `<root>/packs/`.
fn count_packs(root: &Path) -> Result<usize> {
    let dir = root.join(PACKS_SUBDIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error).with_context(|| format!("reading {}", dir.display())),
    };

    let mut count = 0;
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        let is_toml = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
        if path.is_file() && is_toml && !is_hidden(&path) {
            count += 1;
        }
    }
    Ok(count)
}

/// `true` for dot-files and dot-directories.
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

/// `true` when `path` is the `packs/` directory.
fn is_packs_dir(path: &Path) -> bool {
    path.file_name() == Some(OsStr::new(PACKS_SUBDIR))
}

#[cfg(test)]
mod tests;
