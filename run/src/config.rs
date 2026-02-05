//! Configuration file (Runfile) discovery and loading.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

thread_local! {
    static CUSTOM_RUNFILE_PATH: RefCell<Option<PathBuf>> = RefCell::new(None);
}

/// Set a custom runfile path for the current thread
pub fn set_custom_runfile_path(path: Option<PathBuf>) {
    CUSTOM_RUNFILE_PATH.with(|p| {
        *p.borrow_mut() = path;
    });
}

/// Get the custom runfile path if set
fn get_custom_runfile_path() -> Option<PathBuf> {
    CUSTOM_RUNFILE_PATH.with(|p| p.borrow().clone())
}

/// Get the user's home directory in a cross-platform way.
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
pub fn load_from_path(path: &PathBuf) -> Option<String> {
    let runfile_path = if path.is_dir() {
        path.join("Runfile")
    } else {
        path.clone()
    };
    
    if runfile_path.exists() {
        fs::read_to_string(&runfile_path).ok()
    } else {
        None
    }
}

/// Search for a Runfile in the current directory or upwards, then fallback to ~/.runfile.
/// Returns Some(content) if a file is found (even if empty), or None if no file exists.
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
            current_dir == *home
                || current_dir == PathBuf::from("/")
                || current_dir == PathBuf::from("\\")
        } else {
            current_dir == PathBuf::from("/") || current_dir == PathBuf::from("\\")
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
fn load_home_runfile() -> Option<String> {
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
pub fn load_config_or_exit() -> String {
    load_config().unwrap_or_else(|| crate::fatal_error(NO_RUNFILE_ERROR))
}

/// Find the path to the Runfile without loading its contents.
/// Uses the same search logic as load_config().
/// Returns Some(path) if found, None otherwise.
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
            current_dir == *home
                || current_dir == PathBuf::from("/")
                || current_dir == PathBuf::from("\\")
        } else {
            current_dir == PathBuf::from("/") || current_dir == PathBuf::from("\\")
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

