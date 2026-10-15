//! Shared sandbox and helpers for the `skm` end-to-end tests.
//!
//! Every test runs the real binary with `XDG_CONFIG_HOME` and `HOME` pointing into a
//! temporary sandbox, so the user's real configuration is never read nor written.
//! The helpers are shared by several test binaries, hence the dead-code allowance.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// Temporary directory holding an isolated config directory, home and stores.
pub struct Sandbox {
    _tmp: TempDir,
    pub root: PathBuf,
    config_home: PathBuf,
    pub home: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let root = tmp.path().to_path_buf();
        let config_home = root.join("config");
        let home = root.join("home");
        fs::create_dir_all(&config_home).expect("create config home");
        fs::create_dir_all(&home).expect("create fake home");
        Self {
            _tmp: tmp,
            root,
            config_home,
            home,
        }
    }

    /// `skm` wired to this sandbox.
    pub fn skm(&self) -> Command {
        let mut command = Command::cargo_bin("skm").expect("skm binary");
        command
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("HOME", &self.home)
            .env("NO_COLOR", "1")
            .current_dir(&self.root);
        command
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_home.join("skm").join("config.toml")
    }

    pub fn config_text(&self) -> String {
        fs::read_to_string(self.config_file()).expect("read config file")
    }

    pub fn write_config(&self, text: &str) {
        let path = self.config_file();
        fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
        fs::write(path, text).expect("write config file");
    }

    /// Create `<store>/<directory>/SKILL.md`.
    pub fn skill(&self, store: &Path, directory: &str, declared_name: Option<&str>) -> PathBuf {
        let dir = store.join(directory);
        fs::create_dir_all(&dir).expect("create skill dir");
        let body = match declared_name {
            Some(name) => format!("---\nname: {name}\ndescription: test skill\n---\n\n# Body\n"),
            None => "# No frontmatter\n".to_owned(),
        };
        fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
        dir
    }

    /// Flat store: two skills, a broken dir, hidden entries, packs and a loose file.
    pub fn flat_store(&self) -> PathBuf {
        let root = self.path("flat-store");
        self.skill(&root, "alpha", Some("alpha-declared"));
        self.skill(&root, "beta", None);
        self.skill(&root, ".hidden", None);
        fs::create_dir_all(root.join("broken")).expect("create broken dir");
        fs::create_dir_all(root.join("packs")).expect("create packs dir");
        fs::write(root.join("packs").join("one.toml"), "").expect("write pack");
        fs::write(root.join("packs").join("two.toml"), "").expect("write pack");
        fs::write(root.join("packs").join("notes.md"), "").expect("write notes");
        fs::write(root.join("loose.md"), "").expect("write loose file");
        root
    }

    /// Catalog store: `skills/` subdirectory with two skills.
    pub fn catalog_store(&self) -> PathBuf {
        let root = self.path("catalog-store");
        let skills = root.join("skills");
        self.skill(&skills, "delta", None);
        self.skill(&skills, "gamma", Some("gamma"));
        fs::create_dir_all(root.join("loose-root-dir")).expect("create loose dir");
        root
    }

    /// Create (once) and return the project directory used by project commands.
    pub fn project(&self) -> PathBuf {
        let dir = self.root.join("project");
        fs::create_dir_all(&dir).expect("create project dir");
        dir
    }

    /// `skm` running inside `dir`, for project commands.
    pub fn skm_in(&self, dir: &Path) -> Command {
        let mut command = self.skm();
        command.current_dir(dir);
        command
    }

    /// Register `path` as the global store through the real binary.
    pub fn register_store(&self, path: &Path) {
        stdout_of(
            self.skm()
                .args(["store", "add", path.to_str().expect("store path is UTF-8")]),
        );
    }
}

/// Exact stdout of a command whose output is `lines` joined by newlines.
pub fn lines(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Split rendered table output into rows of cells (columns are 2+ spaces apart).
pub fn table_cells(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .collect()
}

/// Expected table content, written row by row.
pub fn rows(content: &[&[&str]]) -> Vec<Vec<String>> {
    content
        .iter()
        .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
        .collect()
}

pub fn stdout_of(command: &mut Command) -> String {
    let output = command.output().expect("run skm");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

/// Run a command that must fail; returns its stdout and stderr.
pub fn failure_of(command: &mut Command) -> (String, String) {
    let output = command.output().expect("run skm");
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    (
        String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    )
}

/// Write `text` to `path`, creating parent directories.
pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(path, text).expect("write file");
}

/// Text of a file, with a clear failure when it cannot be read.
pub fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// Manifest path inside a project directory.
pub fn manifest_of(project: &Path) -> PathBuf {
    project.join("skills.toml")
}

/// State file path inside a project directory.
pub fn state_of(project: &Path) -> PathBuf {
    project.join(".skills-manager").join("state.toml")
}

/// Value of the `key = "value"` line of a TOML document.
pub fn value_of(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key} = ")))
        .map(|value| value.trim_matches('"').to_owned())
        .unwrap_or_else(|| panic!("no `{key}` in:\n{text}"))
}

/// Every entry under `root` as sorted relative paths; directories end with `/`.
pub fn tree(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = walkdir::WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .skip(1)
        .map(|entry| {
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("entry lives under the root")
                .to_string_lossy()
                .into_owned();
            if entry.file_type().is_dir() {
                format!("{relative}/")
            } else if entry.file_type().is_symlink() {
                format!("{relative}@")
            } else {
                relative
            }
        })
        .collect();
    found.sort();
    found
}

/// Modification time of `path`, for tests that prove nothing was written.
pub fn mtime(path: &Path) -> std::time::SystemTime {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("cannot stat {}: {error}", path.display()))
        .modified()
        .expect("mtime is available")
}

/// `true` when `path` is a symlink.
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// `true` when `path` carries an execute bit.
#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Make `path` executable, as a store author would.
#[cfg(unix)]
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path).expect("stat file").permissions().mode();
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o755)).expect("chmod file");
}

pub const DEFAULT_CONFIG: &str = "default_target = \".agents/skills\"\n\
                              \n\
                              [agents]\n\
                              opencode = \".opencode/skills\"\n\
                              \n\
                              [watch]\n\
                              debounce_ms = 500\n";
