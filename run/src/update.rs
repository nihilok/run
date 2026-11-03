//! Self-update support (`run --update`).
//!
//! Checks the latest GitHub Release and upgrades `run` using whichever
//! installation mechanism owns the current executable (Homebrew, Scoop,
//! AUR/pacman, Cargo, or direct GitHub Release binary download).

use crate::config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_RELEASES_API_URL: &str = "https://api.github.com/repos/nihilok/run/releases/latest";
const RELEASES_DOWNLOAD_BASE: &str = "https://github.com/nihilok/run/releases/download";

/// How the running `run` executable was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    Homebrew,
    Scoop,
    Aur,
    Cargo,
    Standalone,
}

/// Entry point for `run --update`.
pub fn run_update() {
    if let Err(err) = update_inner() {
        crate::fatal_error(&format!("error: {err}"));
    }
}

fn update_inner() -> Result<(), String> {
    let latest = fetch_latest_version()?;
    if !is_newer_version(CURRENT_VERSION, &latest) {
        println!("run {CURRENT_VERSION} is already up to date.");
        return Ok(());
    }

    let exe_path = resolve_current_exe()?;
    let method = detect_install_method(&exe_path);

    println!("Updating run {CURRENT_VERSION} -> {latest} ({method})...");

    match method {
        InstallMethod::Homebrew => run_status_cmd("brew", &["upgrade", "nihilok/tap/runtool"]),
        InstallMethod::Scoop => run_status_cmd("scoop", &["update", "runtool"]),
        InstallMethod::Aur => update_via_aur(),
        InstallMethod::Cargo => {
            let crate_name = exe_path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|&s| s == "runtool")
                .unwrap_or("run");
            run_status_cmd("cargo", &["install", crate_name])
        }
        InstallMethod::Standalone => update_standalone_binary(&exe_path, &latest),
    }?;

    println!("✓ Updated run to {latest}");
    Ok(())
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Homebrew => "Homebrew",
            Self::Scoop => "Scoop",
            Self::Aur => "AUR",
            Self::Cargo => "Cargo",
            Self::Standalone => "GitHub Release binary",
        };
        f.write_str(label)
    }
}

fn resolve_current_exe() -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("failed to locate current executable: {e}"))?;
    Ok(fs::canonicalize(&exe).unwrap_or(exe))
}

/// Determine how `exe_path` was installed based on its canonical path and OS queries.
#[must_use]
pub fn detect_install_method(exe_path: &Path) -> InstallMethod {
    let cargo_bin = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| config::get_home_dir().map(|h| h.join(".cargo")))
        .map(|p| p.join("bin"));

    detect_install_method_from_parts(exe_path, cargo_bin.as_deref(), is_owned_by_pacman(exe_path))
}

#[must_use]
fn detect_install_method_from_parts(
    exe_path: &Path,
    cargo_bin: Option<&Path>,
    owned_by_pacman: bool,
) -> InstallMethod {
    let normalised = exe_path.to_string_lossy().replace('\\', "/");
    let lower = normalised.to_ascii_lowercase();

    if normalised.contains("/Cellar/runtool/")
        || normalised.contains("/Caskroom/runtool/")
        || lower.contains("/homebrew/")
        || lower.contains("/linuxbrew/")
    {
        return InstallMethod::Homebrew;
    }

    if lower.contains("/scoop/apps/runtool/") || lower.contains("/scoop/shims/") {
        return InstallMethod::Scoop;
    }

    if let Some(bin_dir) = cargo_bin
        && exe_path.starts_with(bin_dir)
    {
        return InstallMethod::Cargo;
    }

    if owned_by_pacman {
        return InstallMethod::Aur;
    }

    InstallMethod::Standalone
}

fn is_owned_by_pacman(exe_path: &Path) -> bool {
    if !cfg!(target_os = "linux") || which::which("pacman").is_err() {
        return false;
    }
    Command::new("pacman")
        .arg("-Qo")
        .arg(exe_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn fetch_latest_version() -> Result<String, String> {
    if let Ok(override_ver) = std::env::var("RUN_UPDATE_LATEST_VERSION") {
        let trimmed = override_ver.trim().trim_start_matches('v').to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let api_url = std::env::var("RUN_UPDATE_API_URL")
        .unwrap_or_else(|_| DEFAULT_RELEASES_API_URL.to_string());

    let body = if which::which("curl").is_ok() {
        let output = Command::new("curl")
            .args([
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                &format!("User-Agent: run/{CURRENT_VERSION}"),
                &api_url,
            ])
            .output()
            .map_err(|e| format!("failed to run curl: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "failed to fetch latest release from {api_url}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else if cfg!(target_os = "windows") {
        let script = format!(
            "(Invoke-RestMethod -Uri '{api_url}' -Headers @{{'User-Agent'='run/{CURRENT_VERSION}'}}) | ConvertTo-Json -Compress"
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("failed to run powershell: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "failed to fetch latest release from {api_url}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else {
        return Err("curl is required to check for updates".to_string());
    };

    parse_release_tag(&body)
}

fn parse_release_tag(json_body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(json_body)
        .map_err(|e| format!("invalid release metadata JSON: {e}"))?;
    let tag = value
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "release metadata missing 'tag_name'".to_string())?;
    let version = tag.trim().trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err("empty version tag in release metadata".to_string());
    }
    Ok(version)
}

/// Returns `true` if `candidate` is strictly newer than `current`.
#[must_use]
pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    match (parse_semver(current), parse_semver(candidate)) {
        (Some(cur), Some(cand)) => cand > cur,
        _ => candidate.trim_start_matches('v') != current.trim_start_matches('v'),
    }
}

fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let clean = version.trim().trim_start_matches('v');
    let core = clean.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn run_status_cmd(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("failed to execute '{program}': {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "'{program} {}' exited with status {status}",
            args.join(" ")
        ))
    }
}

fn update_via_aur() -> Result<(), String> {
    for helper in ["paru", "yay"] {
        if which::which(helper).is_ok() {
            return run_status_cmd(helper, &["-S", "runtool"]);
        }
    }
    Err(
        "run was installed via pacman/AUR, but neither 'paru' nor 'yay' was found in PATH. \
         Update manually with your AUR helper (e.g. 'yay -S runtool' or 'paru -S runtool')."
            .to_string(),
    )
}

fn release_target_and_ext() -> Result<(&'static str, &'static str), String> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Ok(("x86_64-unknown-linux-gnu", "tar.gz")),
        ("x86_64", "macos") => Ok(("x86_64-apple-darwin", "tar.gz")),
        ("aarch64", "macos") => Ok(("aarch64-apple-darwin", "tar.gz")),
        ("x86_64", "windows") => Ok(("x86_64-pc-windows-msvc", "zip")),
        (arch, os) => Err(format!(
            "no prebuilt release binary available for {arch}-{os}; update with 'cargo install run'"
        )),
    }
}

fn update_standalone_binary(exe_path: &Path, version: &str) -> Result<(), String> {
    let (target, ext) = release_target_and_ext()?;
    let base_url = std::env::var("RUN_UPDATE_DOWNLOAD_BASE")
        .unwrap_or_else(|_| RELEASES_DOWNLOAD_BASE.to_string());
    let archive_name = format!("run-{target}.{ext}");
    let url = format!("{base_url}/v{version}/{archive_name}");

    let staging_dir = std::env::temp_dir().join(format!("run-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("failed to create staging directory: {e}"))?;

    let result = download_and_replace(&url, &staging_dir, &archive_name, ext, exe_path);
    let _ = fs::remove_dir_all(&staging_dir);
    result
}

fn download_and_replace(
    url: &str,
    staging_dir: &Path,
    archive_name: &str,
    ext: &str,
    exe_path: &Path,
) -> Result<(), String> {
    let archive_path = staging_dir.join(archive_name);
    let archive_str = archive_path.to_string_lossy();

    if which::which("curl").is_ok() {
        run_status_cmd("curl", &["-fsSL", "-o", &archive_str, url])?;
    } else if cfg!(target_os = "windows") {
        let script = format!("Invoke-WebRequest -Uri '{url}' -OutFile '{archive_str}'");
        run_status_cmd("powershell", &["-NoProfile", "-Command", &script])?;
    } else {
        return Err("curl is required to download release archive".to_string());
    }

    let staging_str = staging_dir.to_string_lossy();
    if ext == "tar.gz" {
        run_status_cmd("tar", &["-xzf", &archive_str, "-C", &staging_str])?;
    } else if cfg!(target_os = "windows") {
        let script =
            format!("Expand-Archive -Path '{archive_str}' -DestinationPath '{staging_str}' -Force");
        run_status_cmd("powershell", &["-NoProfile", "-Command", &script])?;
    } else {
        run_status_cmd("unzip", &["-q", &archive_str, "-d", &staging_str])?;
    }

    let binary_name = if cfg!(target_os = "windows") {
        "run.exe"
    } else {
        "run"
    };
    let extracted_bin = staging_dir.join(binary_name);
    if !extracted_bin.exists() {
        return Err(format!(
            "downloaded archive did not contain expected '{binary_name}' binary"
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&extracted_bin, perms)
            .map_err(|e| format!("failed to set executable permissions: {e}"))?;
    }

    replace_executable(&extracted_bin, exe_path)
}

fn replace_executable(extracted_bin: &Path, exe_path: &Path) -> Result<(), String> {
    let parent = exe_path
        .parent()
        .ok_or_else(|| "invalid executable path".to_string())?;
    let sibling_new = parent.join(format!(".run.update.{}", std::process::id()));

    fs::copy(extracted_bin, &sibling_new).map_err(|e| {
        format!(
            "failed to write updated binary next to '{}': {e} (check permissions)",
            exe_path.display()
        )
    })?;

    #[cfg(windows)]
    {
        let backup = exe_path.with_extension("old.exe");
        let _ = fs::remove_file(&backup);
        let _ = fs::rename(exe_path, &backup);
    }

    if let Err(e) = fs::rename(&sibling_new, exe_path) {
        let _ = fs::remove_file(&sibling_new);
        return Err(format!("failed to replace '{}': {e}", exe_path.display()));
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.7.0", "0.8.0"));
        assert!(is_newer_version("0.8.0", "0.8.1"));
        assert!(is_newer_version("0.8.9", "0.10.0"));
        assert!(!is_newer_version("0.8.0", "0.8.0"));
        assert!(!is_newer_version("0.8.0", "0.7.1"));
        assert!(is_newer_version("v0.7.1", "v0.8.0"));
    }

    #[test]
    fn test_parse_release_tag() {
        let json = r#"{"tag_name": "v0.8.0", "name": "v0.8.0"}"#;
        assert_eq!(parse_release_tag(json).unwrap(), "0.8.0");
    }

    #[test]
    fn test_detect_install_method() {
        assert_eq!(
            detect_install_method_from_parts(
                Path::new("/opt/homebrew/Cellar/runtool/0.8.0/bin/run"),
                None,
                false
            ),
            InstallMethod::Homebrew
        );
        assert_eq!(
            detect_install_method_from_parts(
                Path::new("C:\\Users\\me\\scoop\\apps\\runtool\\current\\run.exe"),
                None,
                false
            ),
            InstallMethod::Scoop
        );
        assert_eq!(
            detect_install_method_from_parts(
                Path::new("/home/me/.cargo/bin/run"),
                Some(Path::new("/home/me/.cargo/bin")),
                false
            ),
            InstallMethod::Cargo
        );
        assert_eq!(
            detect_install_method_from_parts(Path::new("/usr/bin/run"), None, true),
            InstallMethod::Aur
        );
        assert_eq!(
            detect_install_method_from_parts(Path::new("/usr/local/bin/run"), None, false),
            InstallMethod::Standalone
        );
    }
}
