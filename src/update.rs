//! Self-update capability for `skm` from GitHub releases.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::output::Printer;

/// GitHub repository owner and name.
const REPO: &str = "fusoras/skills-manager";

/// Current package version at compile time.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Determine the target triple matching the current architecture.
pub fn target_triple() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64-unknown-linux-gnu"),
        "aarch64" => Ok("aarch64-unknown-linux-musl"),
        other => bail!(
            "unsupported architecture '{other}'; supported architectures are x86_64 and aarch64"
        ),
    }
}

/// Verify that required host tools (`curl`, `tar`) are available in PATH.
pub fn check_prerequisites() -> Result<()> {
    for tool in ["curl", "tar"] {
        let ok = Command::new(tool)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            bail!("required tool '{tool}' is not installed or not in PATH");
        }
    }
    Ok(())
}

/// Resolve the latest release tag from GitHub.
pub fn fetch_latest_tag() -> Result<String> {
    let redirect_url = format!("https://github.com/{REPO}/releases/latest");
    let output = Command::new("curl")
        .args(["-sI", &redirect_url])
        .output()
        .context("failed to query latest release from GitHub via curl")?;

    if !output.status.success() {
        bail!("curl failed while querying latest release from GitHub");
    }

    let headers = String::from_utf8_lossy(&output.stdout);
    for line in headers.lines() {
        let trimmed = line.trim();
        if trimmed.to_ascii_lowercase().starts_with("location:")
            && let Some((_, dest)) = trimmed.split_once(':')
        {
            let dest = dest.trim();
            if let Some(pos) = dest.rfind('/') {
                let tag = dest[pos + 1..].trim();
                if !tag.is_empty() {
                    return Ok(tag.to_owned());
                }
            }
        }
    }

    bail!("could not determine latest release tag from GitHub releases redirect")
}

/// Normalize a version string into a tag name (ensuring leading `v`).
pub fn normalize_tag(version_or_tag: &str) -> String {
    let trimmed = version_or_tag.trim();
    if trimmed.starts_with('v') || trimmed.starts_with('V') {
        trimmed.to_lowercase()
    } else {
        format!("v{trimmed}")
    }
}

/// Extract clean SemVer string from tag (stripping leading `v`).
pub fn clean_version(tag: &str) -> &str {
    tag.strip_prefix(['v', 'V']).unwrap_or(tag)
}

/// Options for `skm update`.
#[derive(Debug, Clone, Default)]
pub struct UpdateOptions<'a> {
    pub tag: Option<&'a str>,
    pub check: bool,
    pub force: bool,
}

/// RAII temporary directory guard.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "skm-update-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp directory '{}'", path.display()))?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Execute the `update` command.
pub fn execute(out: &mut Printer, options: &UpdateOptions) -> Result<()> {
    check_prerequisites()?;

    let target_tag = match options.tag {
        Some(t) => normalize_tag(t),
        None => fetch_latest_tag()?,
    };

    let target_ver = clean_version(&target_tag);
    let is_current = target_ver == CURRENT_VERSION;

    if options.check {
        if is_current {
            out.line(&format!("skm is up to date (v{CURRENT_VERSION})."))?;
        } else {
            out.line(&format!(
                "An update is available: v{CURRENT_VERSION} -> {target_tag}. Run 'skm update' to install."
            ))?;
        }
        return Ok(());
    }

    if is_current && !options.force && options.tag.is_none() {
        out.line(&format!(
            "skm is already up to date (v{CURRENT_VERSION}). Pass --force to reinstall."
        ))?;
        return Ok(());
    }

    let triple = target_triple()?;
    let asset_name = format!("skm-{triple}.tar.gz");
    let download_url =
        format!("https://github.com/{REPO}/releases/download/{target_tag}/{asset_name}");

    let tmp_dir = TempDir::new()?;
    let tar_path = tmp_dir.path().join(&asset_name);

    out.line(&format!(
        "Downloading skm {target_tag} ({triple}) from GitHub..."
    ))?;

    let curl_status = Command::new("curl")
        .args(["-fsSL", &download_url, "-o"])
        .arg(&tar_path)
        .status()
        .with_context(|| format!("failed to download from '{download_url}'"))?;

    if !curl_status.success() {
        bail!("failed to download release asset from '{download_url}'");
    }

    let tar_status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tar_path)
        .arg("-C")
        .arg(tmp_dir.path())
        .status()
        .context("failed to extract release archive with tar")?;

    if !tar_status.success() {
        bail!("failed to unpack release archive '{asset_name}'");
    }

    let extracted_bin = tmp_dir.path().join("skm");
    if !extracted_bin.is_file() {
        bail!("extracted archive did not contain binary 'skm'");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&extracted_bin, fs::Permissions::from_mode(0o755))
            .context("failed to set executable permissions on downloaded binary")?;
    }

    let current_exe = std::env::current_exe().context("resolving current executable path")?;
    let dest_exe = fs::canonicalize(&current_exe).unwrap_or(current_exe);

    replace_binary(&extracted_bin, &dest_exe)?;

    out.line(&format!(
        "Successfully updated skm to {target_tag} ({})",
        dest_exe.display()
    ))?;

    Ok(())
}

/// Atomically replace `target_exe` with `new_bin`.
fn replace_binary(new_bin: &Path, target_exe: &Path) -> Result<()> {
    let dest_dir = target_exe
        .parent()
        .with_context(|| format!("binary path '{}' has no parent directory", target_exe.display()))?;

    let temp_dest = dest_dir.join(format!(".skm-update-{}", std::process::id()));

    fs::copy(new_bin, &temp_dest).with_context(|| {
        format!(
            "cannot write to '{}' (permission denied; try running with sudo)",
            dest_dir.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(&temp_dest, fs::Permissions::from_mode(0o755)) {
            let _ = fs::remove_file(&temp_dest);
            return Err(err).context("failed to set executable permissions on temp binary");
        }
    }

    if let Err(err) = fs::rename(&temp_dest, target_exe) {
        let _ = fs::remove_file(&temp_dest);
        return Err(err).with_context(|| {
            format!(
                "cannot replace '{}' (permission denied; try running with sudo)",
                target_exe.display()
            )
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_triple_returns_valid_supported_target() {
        let triple = target_triple().unwrap();
        assert!(
            triple == "x86_64-unknown-linux-gnu" || triple == "aarch64-unknown-linux-musl",
            "unexpected triple: {triple}"
        );
    }

    #[test]
    fn normalize_tag_handles_with_and_without_v() {
        assert_eq!(normalize_tag("0.1.0"), "v0.1.0");
        assert_eq!(normalize_tag("v0.1.0"), "v0.1.0");
        assert_eq!(normalize_tag("V1.2.3"), "v1.2.3");
    }

    #[test]
    fn clean_version_strips_v_prefix() {
        assert_eq!(clean_version("v0.1.0"), "0.1.0");
        assert_eq!(clean_version("V0.1.0"), "0.1.0");
        assert_eq!(clean_version("0.1.0"), "0.1.0");
    }

    #[test]
    fn replace_binary_atomically_swaps_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("skm");
        let new_file = tmp.path().join("skm-new");

        fs::write(&target, "old binary").unwrap();
        fs::write(&new_file, "new binary").unwrap();

        replace_binary(&new_file, &target).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new binary");
    }
}
