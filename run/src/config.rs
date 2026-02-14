//! Configuration file (Runfile) discovery and loading.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

thread_local! {
    static CUSTOM_RUNFILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static MCP_OUTPUT_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

static MCP_OUTPUT_ENV: OnceLock<Option<PathBuf>> = OnceLock::new();
const MCP_OUTPUT_ENV_VAR: &str = "RUN_MCP_OUTPUT_DIR";

/// Set a custom runfile path for the current thread
pub fn set_custom_runfile_path(path: Option<PathBuf>) {
    CUSTOM_RUNFILE_PATH.with(|p| {
        *p.borrow_mut() = path;
    });
}

/// Get the custom runfile path if set
#[must_use]
pub fn get_custom_runfile_path() -> Option<PathBuf> {
    CUSTOM_RUNFILE_PATH.with(|p| p.borrow().clone())
}

/// Set the MCP output directory for the current thread
pub fn set_mcp_output_dir(path: Option<PathBuf>) {
    MCP_OUTPUT_DIR.with(|p| {
        *p.borrow_mut() = path;
    });
}

fn mcp_output_dir_from_env() -> Option<PathBuf> {
    MCP_OUTPUT_ENV
        .get_or_init(|| std::env::var_os(MCP_OUTPUT_ENV_VAR).map(PathBuf::from))
        .clone()
}

fn resolve_runfile_dir() -> Option<PathBuf> {
    // Prefer explicit custom path if set
    if let Some(custom_path) = get_custom_runfile_path() {
        return Some(if custom_path.is_dir() {
            custom_path
        } else {
            custom_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });
    }

    find_runfile_path().and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Compute (and memoize) the MCP output directory. Prefers env override, then Runfile dir, then temp.
#[must_use]
pub fn ensure_mcp_output_dir() -> PathBuf {
    MCP_OUTPUT_DIR.with(|p| {
        if p.borrow().is_none() {
            let base_dir = mcp_output_dir_from_env()
                .or_else(resolve_runfile_dir)
                .unwrap_or_else(std::env::temp_dir);
            let dir = if base_dir
                .file_name()
                .is_some_and(|name| name == ".run-output")
            {
                base_dir
            } else {
                base_dir.join(".run-output")
            };
            *p.borrow_mut() = Some(dir);
        }
        // SAFETY: We just initialized the value above if it was None
        match p.borrow().clone() {
            Some(dir) => dir,
            None => unreachable!("MCP output directory was just initialized"),
        }
    })
}

/// Check if MCP output directory is configured or derivable (env or Runfile)
#[must_use]
pub fn is_mcp_output_configured() -> bool {
    MCP_OUTPUT_DIR.with(|p| p.borrow().is_some()) || mcp_output_dir_from_env().is_some()
}

/// Get the MCP output directory if set, otherwise derives it and memoizes
#[must_use]
pub fn get_mcp_output_dir() -> PathBuf {
    ensure_mcp_output_dir()
}

/// Get the user's home directory in a cross-platform way.
#[must_use]
pub fn get_home_dir() -> Option<PathBuf> {
    // Try HOME first (Unix-like systems)
    if let Some(home) = std::env::var_os("HOME") {
        return Some(PathBuf::from(home));
    }

    // Try USERPROFILE (Windows)
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(userprofile));
    }

    // Try HOMEDRIVE + HOMEPATH (older Windows)
    if let (Some(homedrive), Some(homepath)) =
        (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
    {
        let mut path = PathBuf::from(homedrive);
        path.push(homepath);
        return Some(path);
    }

    None
}

/// Load a Runfile from a specific path (file or directory)
/// If path is a directory, looks for Runfile inside it
/// Returns Some(content) if found, None otherwise
#[must_use]
pub fn load_from_path(path: &Path) -> Option<String> {
    let runfile_path = if path.is_dir() {
        path.join("Runfile")
    } else {
        path.to_path_buf()
    };

    if runfile_path.exists() {
        fs::read_to_string(&runfile_path).ok()
    } else {
        None
    }
}

/// Search for a Runfile in the current directory or upwards, then fallback to ~/.runfile.
/// Returns Some(content) if a file is found (even if empty), or None if no file exists.
#[must_use]
pub fn load_config() -> Option<String> {
    // First, check if a custom runfile path is set
    if let Some(custom_path) = get_custom_runfile_path() {
        return load_from_path(&custom_path);
    }

    // Start from the current directory and search upwards
    let mut current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            // If we can't get current dir, fall back to home directory only
            return load_home_runfile();
        }
    };

    // Get home directory for boundary check
    let home_dir = get_home_dir();

    // Search upwards from current directory
    loop {
        let runfile_path = current_dir.join("Runfile");
        if runfile_path.exists() {
            // File exists, read it (even if empty)
            if let Ok(content) = fs::read_to_string(&runfile_path) {
                return Some(content);
            }
        }

        // Check if we've reached the home directory or root
        let reached_boundary = if let Some(ref home) = home_dir {
            current_dir == *home || *current_dir == *"/" || *current_dir == *"\\"
        } else {
            *current_dir == *"/" || *current_dir == *"\\"
        };

        if reached_boundary {
            break;
        }

        // Move up one directory
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break, // Reached root
        }
    }

    // Finally, try ~/.runfile as a fallback
    load_home_runfile()
}

/// Load ~/.runfile from the user's home directory.
/// Returns Some(content) if found, or None otherwise.
#[must_use]
pub fn load_home_runfile() -> Option<String> {
    if let Some(home) = get_home_dir() {
        let runfile_path = home.join(".runfile");
        if runfile_path.exists()
            && let Ok(content) = fs::read_to_string(runfile_path)
        {
            return Some(content);
        }
    }
    None
}

/// Error message when no Runfile is found.
pub const NO_RUNFILE_ERROR: &str =
    "Error: No Runfile found. Create ~/.runfile or ./Runfile to define functions.";

/// Load config or exit with an error message.
#[must_use]
pub fn load_config_or_exit() -> String {
    load_config().unwrap_or_else(|| crate::fatal_error(NO_RUNFILE_ERROR))
}

/// Find the path to the Runfile without loading its contents.
/// Uses the same search logic as `load_config()`.
/// Returns Some(path) if found, None otherwise.
#[must_use]
pub fn find_runfile_path() -> Option<PathBuf> {
    // First, check if a custom runfile path is set
    if let Some(custom_path) = get_custom_runfile_path() {
        if custom_path.is_dir() {
            let runfile_path = custom_path.join("Runfile");
            if runfile_path.exists() {
                return Some(runfile_path);
            }
        } else if custom_path.exists() {
            return Some(custom_path);
        }
        return None;
    }

    // Start from the current directory and search upwards
    let mut current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            // If we can't get current dir, fall back to home directory only
            return find_home_runfile_path();
        }
    };

    // Get home directory for boundary check
    let home_dir = get_home_dir();

    // Search upwards from current directory
    loop {
        let runfile_path = current_dir.join("Runfile");
        if runfile_path.exists() {
            return Some(runfile_path);
        }

        // Check if we've reached the home directory or root
        let reached_boundary = if let Some(ref home) = home_dir {
            current_dir == *home || *current_dir == *"/" || *current_dir == *"\\"
        } else {
            *current_dir == *"/" || *current_dir == *"\\"
        };

        if reached_boundary {
            break;
        }

        // Move up one directory
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break, // Reached root
        }
    }

    // Finally, try ~/.runfile as a fallback
    find_home_runfile_path()
}

/// Find the path to ~/.runfile if it exists.
fn find_home_runfile_path() -> Option<PathBuf> {
    if let Some(home) = get_home_dir() {
        let runfile_path = home.join(".runfile");
        if runfile_path.exists() {
            return Some(runfile_path);
        }
    }
    None
}

/// Find the project Runfile path (searching upward from cwd).
/// Returns None if no project Runfile is found (will not return ~/.runfile).
#[must_use]
pub fn find_project_runfile_path() -> Option<PathBuf> {
    // First, check if a custom runfile path is set
    if let Some(custom_path) = get_custom_runfile_path() {
        if custom_path.is_dir() {
            let runfile_path = custom_path.join("Runfile");
            if runfile_path.exists() {
                return Some(runfile_path);
            }
        } else if custom_path.exists() {
            return Some(custom_path);
        }
        return None;
    }

    // Start from the current directory and search upwards
    let mut current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    // Get home directory for boundary check
    let home_dir = get_home_dir();

    // Search upwards from current directory
    loop {
        let runfile_path = current_dir.join("Runfile");
        if runfile_path.exists() {
            return Some(runfile_path);
        }

        // Check if we've reached the home directory or root
        let reached_boundary = if let Some(ref home) = home_dir {
            current_dir == *home || *current_dir == *"/" || *current_dir == *"\\"
        } else {
            *current_dir == *"/" || *current_dir == *"\\"
        };

        if reached_boundary {
            break;
        }

        // Move up one directory
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break, // Reached root
        }
    }

    None
}

/// Load and merge both global (~/.runfile) and project (./Runfile) configurations.
/// Returns the merged content with project functions taking precedence over global ones.
/// Also returns metadata about which files were loaded.
///
/// Special cases:
/// 1. If a custom runfile is set via --runfile flag, ONLY that file is used (no merging).
/// 2. If `RUN_NO_GLOBAL_MERGE` env var is set, global runfile is not merged (for tests).
///
/// The merge strategy is simple: concatenate global content first, then project content.
/// When parsed, later function definitions naturally override earlier ones in the interpreter.
#[must_use]
pub fn load_merged_config() -> Option<(String, MergeMetadata)> {
    // If a custom runfile is explicitly specified, use ONLY that file (don't merge)
    if let Some(custom_path) = get_custom_runfile_path() {
        return load_from_path(&custom_path).map(|content| {
            (
                content,
                MergeMetadata {
                    has_global: false,
                    has_project: true,
                },
            )
        });
    }

    // Check if global merging is disabled (for tests)
    let disable_global_merge = std::env::var("RUN_NO_GLOBAL_MERGE").is_ok();

    // Load project runfile
    let project_content = if let Some(project_path) = find_project_runfile_path() {
        fs::read_to_string(&project_path).ok()
    } else {
        None
    };

    // Load global runfile
    // If RUN_NO_GLOBAL_MERGE is set AND we have a project runfile, don't merge global
    // But if there's no project runfile, still allow global as fallback
    let global_content = if disable_global_merge && project_content.is_some() {
        None
    } else {
        load_home_runfile()
    };

    match (global_content, project_content) {
        (None, None) => None,
        (Some(global), None) => Some((
            global,
            MergeMetadata {
                has_global: true,
                has_project: false,
            },
        )),
        (None, Some(project)) => Some((
            project,
            MergeMetadata {
                has_global: false,
                has_project: true,
            },
        )),
        (Some(global), Some(project)) => {
            // Concatenate with global first, project second
            // Add a newline separator to ensure proper parsing
            let merged = format!("{global}\n{project}");
            Some((
                merged,
                MergeMetadata {
                    has_global: true,
                    has_project: true,
                },
            ))
        }
    }
}

/// Metadata about which runfiles were loaded during a merge.
#[derive(Debug, Clone, Copy)]
pub struct MergeMetadata {
    pub has_global: bool,
    pub has_project: bool,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_get_home_dir_returns_some() {
        // On any system with HOME or USERPROFILE set, should return Some
        let result = get_home_dir();
        assert!(result.is_some());
    }

    #[test]
    #[serial]
    fn test_set_and_get_custom_runfile_path() {
        let original = get_custom_runfile_path();

        set_custom_runfile_path(Some(PathBuf::from("/tmp/test_runfile")));
        assert_eq!(
            get_custom_runfile_path(),
            Some(PathBuf::from("/tmp/test_runfile"))
        );

        set_custom_runfile_path(None);
        assert_eq!(get_custom_runfile_path(), None);

        // Restore
        set_custom_runfile_path(original);
    }

    #[test]
    fn test_load_from_path_file_exists() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let runfile = temp.path().join("Runfile");
        fs::write(&runfile, "greet = echo hello").expect("Failed to write");

        let result = load_from_path(&runfile);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "greet = echo hello");
    }

    #[test]
    fn test_load_from_path_directory() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let runfile = temp.path().join("Runfile");
        fs::write(&runfile, "build = cargo build").expect("Failed to write");

        let result = load_from_path(temp.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "build = cargo build");
    }

    #[test]
    fn test_load_from_path_nonexistent() {
        let result = load_from_path(Path::new("/nonexistent/path/Runfile"));
        assert!(result.is_none());
    }

    #[test]
    fn test_load_from_path_dir_without_runfile() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let result = load_from_path(temp.path());
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_set_and_get_mcp_output_dir() {
        // Reset thread-local state
        set_mcp_output_dir(None);

        set_mcp_output_dir(Some(PathBuf::from("/tmp/mcp_test")));
        let dir = ensure_mcp_output_dir();
        assert_eq!(dir, PathBuf::from("/tmp/mcp_test"));

        // Clean up
        set_mcp_output_dir(None);
    }

    #[test]
    #[serial]
    fn test_ensure_mcp_output_dir_fallback() {
        // Reset to force re-computation
        set_mcp_output_dir(None);
        set_custom_runfile_path(None);

        let dir = ensure_mcp_output_dir();
        // Should end with .run-output
        assert!(
            dir.to_string_lossy().contains(".run-output"),
            "MCP output dir should contain .run-output, got: {}",
            dir.display()
        );

        // Clean up
        set_mcp_output_dir(None);
    }

    #[test]
    #[serial]
    fn test_load_config_with_custom_path() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let runfile = temp.path().join("Runfile");
        fs::write(&runfile, "custom = echo custom").expect("Failed to write");

        let original = get_custom_runfile_path();
        set_custom_runfile_path(Some(runfile));

        let result = load_config();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "custom = echo custom");

        // Restore
        set_custom_runfile_path(original);
    }

    #[test]
    #[serial]
    fn test_find_runfile_path_with_custom_path() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let runfile = temp.path().join("Runfile");
        fs::write(&runfile, "test = echo test").expect("Failed to write");

        let original = get_custom_runfile_path();
        set_custom_runfile_path(Some(runfile.clone()));

        let result = find_runfile_path();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), runfile);

        // Restore
        set_custom_runfile_path(original);
    }

    #[test]
    #[serial]
    fn test_find_runfile_path_custom_nonexistent() {
        let original = get_custom_runfile_path();
        set_custom_runfile_path(Some(PathBuf::from("/nonexistent/Runfile")));

        let result = find_runfile_path();
        assert!(result.is_none());

        set_custom_runfile_path(original);
    }

    #[test]
    #[serial]
    fn test_load_merged_config_custom_path() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let runfile = temp.path().join("Runfile");
        fs::write(&runfile, "merged = echo merged").expect("Failed to write");

        let original = get_custom_runfile_path();
        set_custom_runfile_path(Some(runfile));

        let result = load_merged_config();
        assert!(result.is_some());
        let (content, metadata) = result.unwrap();
        assert_eq!(content, "merged = echo merged");
        assert!(!metadata.has_global);
        assert!(metadata.has_project);

        set_custom_runfile_path(original);
    }

    #[test]
    fn test_no_runfile_error_message() {
        assert!(NO_RUNFILE_ERROR.contains("No Runfile found"));
    }
}
