//! CLI flag tests (--version, --list, --help, --inspect, --generate-completion, --install-completion)

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::process::Command;

/// Helper to create a Command with test environment
/// Sets `RUN_NO_GLOBAL_MERGE` to isolate tests from user's ~/.runfile
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

#[test]
fn test_update_flag_already_up_to_date() {
    let binary = get_binary_path();
    let output = test_command_local(&binary)
        .arg("--update")
        .env("RUN_UPDATE_LATEST_VERSION", PKG_VERSION)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("already up to date"),
        "Expected 'already up to date', got: {stdout}"
    );
}

#[cfg(unix)]
#[test]
fn test_update_flag_replaces_standalone_binary() {
    use std::fs;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Copy the current binary to a standalone path outside ~/.cargo/bin
    let standalone_bin = temp_dir.path().join("run");
    fs::copy(&binary, &standalone_bin).expect("copy binary");

    // Prepare a mock release archive at <temp>/v99.0.0/run-<target>.tar.gz
    let target = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        _ => return,
    };
    let release_dir = temp_dir.path().join("v99.0.0");
    let payload_dir = temp_dir.path().join("payload");
    fs::create_dir_all(&release_dir).unwrap();
    fs::create_dir_all(&payload_dir).unwrap();

    let mock_bin = payload_dir.join("run");
    fs::write(&mock_bin, "#!/bin/sh\necho \"run 99.0.0\"\n").unwrap();
    let archive_path = release_dir.join(format!("run-{target}.tar.gz"));
    let tar_status = Command::new("tar")
        .args([
            "-czf",
            archive_path.to_str().unwrap(),
            "-C",
            payload_dir.to_str().unwrap(),
            "run",
        ])
        .status()
        .expect("create tar.gz");
    assert!(tar_status.success());

    let download_base = format!("file://{}", temp_dir.path().display());
    let output = Command::new(&standalone_bin)
        .arg("--update")
        .env("RUN_NO_GLOBAL_MERGE", "1")
        .env("RUN_UPDATE_LATEST_VERSION", "99.0.0")
        .env("RUN_UPDATE_DOWNLOAD_BASE", &download_base)
        .output()
        .expect("run --update");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Updated run to 99.0.0"), "stdout: {stdout}");

    // Verify the binary on disk was replaced and is executable
    let ver_output = Command::new(&standalone_bin)
        .arg("--version")
        .output()
        .expect("run replaced binary");
    assert_eq!(
        String::from_utf8_lossy(&ver_output.stdout).trim(),
        "run 99.0.0"
    );
}
