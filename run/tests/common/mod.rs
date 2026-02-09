//! Common test helpers shared across integration tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(dead_code)] // Not all helpers are used by every test file

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the compiled binary path
pub fn get_binary_path() -> PathBuf {
    // Get the directory where cargo places test binaries
    let mut path = env::current_exe().unwrap();
    path.pop(); // Remove test executable name

    // Check if we're in a 'deps' directory (integration tests)
    if path.ends_with("deps") {
        path.pop(); // Go up to debug or release
    }

    path.push("run");

    // If the binary doesn't exist in debug, try building it first
    if !path.exists() {
        // Try to build the binary
        let build_output = Command::new("cargo")
            .args(["build", "--bin", "run"])
            .output()
            .expect("Failed to build binary");

        assert!(build_output.status.success(), 
            "Failed to build run binary: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );
    }

    path
}

/// Helper to create a temporary directory for tests
pub fn create_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

/// Helper to create a Runfile in a directory
pub fn create_runfile(dir: &std::path::Path, content: &str) {
    let runfile_path = dir.join("Runfile");
    fs::write(runfile_path, content).unwrap();
}

/// Helper to check if Python is available on the system
#[allow(dead_code)]
pub fn is_python_available() -> bool {
    which::which("python3").is_ok() || which::which("python").is_ok()
}

/// Helper to check if Node is available on the system
#[allow(dead_code)]
pub fn is_node_available() -> bool {
    which::which("node").is_ok()
}

/// Helper to check if Ruby is available on the system
#[allow(dead_code)]
pub fn is_ruby_available() -> bool {
    which::which("ruby").is_ok()
}

/// Package version for testing --version flag
#[allow(dead_code)]
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Helper to create a Command with test environment
/// Sets `RUN_NO_GLOBAL_MERGE` to isolate tests from user's ~/.runfile
pub fn test_command(binary: &PathBuf) -> Command {
    let mut cmd = Command::new(binary);
    cmd.env("RUN_NO_GLOBAL_MERGE", "1");
    cmd
}
