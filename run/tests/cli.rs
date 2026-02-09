//! CLI flag tests (--version, --list, --help, --inspect, --generate-completion, --install-completion)

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::process::Command;

/// Helper to create a Command with test environment
/// Sets RUN_NO_GLOBAL_MERGE to isolate tests from user's ~/.runfile
fn test_command_local(binary: &std::path::PathBuf) -> Command {
    let mut cmd = Command::new(binary);
    cmd.env("RUN_NO_GLOBAL_MERGE", "1");
    cmd
}

#[test]
fn test_version_flag() {
    let binary = get_binary_path();
    let output = test_command_local(&binary)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(PKG_VERSION));
}

#[test]
fn test_list_flag_no_runfile() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command_local(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No Runfile found"));
}

#[test]
fn test_list_flag_with_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
build() echo "Building..."
test() echo "Testing..."
deploy() echo "Deploying..."
"#,
    );

    let output = test_command_local(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Available functions")); // Changed to match new format
    assert!(stdout.contains("build"));
    assert!(stdout.contains("test"));
    assert!(stdout.contains("deploy"));
}

#[test]
fn test_list_flag_short() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
hello() echo "Hello, World!"
"#,
    );

    let output = test_command_local(&binary)
        .arg("-l")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
}

#[test]
fn test_generate_completion_bash() {
    let binary = get_binary_path();
    let output = test_command_local(&binary)
        .arg("--generate-completion")
        .arg("bash")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#!/usr/bin/env bash"));
    assert!(stdout.contains("_run_complete"));
    assert!(stdout.contains("complete -F _run_complete run"));
}

#[test]
fn test_generate_completion_zsh() {
    let binary = get_binary_path();
    let output = test_command_local(&binary)
        .arg("--generate-completion")
        .arg("zsh")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#compdef run"));
    assert!(stdout.contains("_run"));
}

#[test]
fn test_generate_completion_fish() {
    let binary = get_binary_path();
    let output = test_command_local(&binary)
        .arg("--generate-completion")
        .arg("fish")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Fish completion script"));
    assert!(stdout.contains("complete -c run"));
}

#[test]
fn test_install_completion_zsh() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command_local(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed completion"));
}

#[test]
fn test_install_completion_bash() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command_local(&binary)
        .arg("--install-completion")
        .arg("bash")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed completion"));
}

#[test]
fn test_install_completion_fish() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command_local(&binary)
        .arg("--install-completion")
        .arg("fish")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed completion"));
}

#[test]
fn test_install_completion_auto_detect_fails_with_unknown_shell() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command_local(&binary)
        .arg("--install-completion")
        .env("HOME", temp_dir.path())
        .env("SHELL", "/bin/unknown_shell")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.to_lowercase().contains("could not detect shell"));
}
