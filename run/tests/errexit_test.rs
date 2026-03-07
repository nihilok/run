#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the compiled binary path
fn get_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("run");
    if !path.exists() {
        let build_output = Command::new("cargo")
            .args(["build", "--bin", "run"])
            .output()
            .expect("Failed to build binary");
        assert!(
            build_output.status.success(),
            "Failed to build run binary: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );
    }
    path
}

fn create_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

fn create_runfile(dir: &std::path::Path, content: &str) {
    let runfile_path = dir.join("Runfile");
    fs::write(runfile_path, content).unwrap();
}

#[test]
fn test_errexit_stops_on_failing_nested_call() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // deploy:patch fails, so deploy:homebrew should NOT run
    let runfile = r#"
deploy:patch() {
    echo "patch starting"
    false
    echo "patch done"
}

deploy:homebrew() {
    echo "homebrew deployed"
}

deploy:all() {
    deploy:patch
    deploy:homebrew
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("deploy:all")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "Expected failure but got success. stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("patch starting"),
        "Expected 'patch starting' in output"
    );
    assert!(
        !stdout.contains("homebrew deployed"),
        "deploy:homebrew should NOT have run after deploy:patch failed"
    );
}

#[test]
fn test_noerrexit_allows_continuation() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @noerrexit
lenient() {
    false
    echo "still running"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("lenient")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected success with @noerrexit. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("still running"),
        "Expected 'still running' in output with @noerrexit"
    );
}

#[test]
fn test_show_script_contains_set_e() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
greet() {
    echo "hello"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "greet"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "show-script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Default shell is bash on most platforms, so expect pipefail
    // Accept either set -eo pipefail (bash) or set -e (sh)
    assert!(
        stdout.contains("set -eo pipefail") || stdout.contains("set -e"),
        "Expected 'set -e' in show-script output, got: {stdout}"
    );
}

#[test]
fn test_show_script_bash_has_pipefail() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell bash
greet() {
    echo "hello"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "greet"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set -eo pipefail"),
        "Expected 'set -eo pipefail' for bash, got: {stdout}"
    );
}

#[test]
fn test_show_script_sh_has_set_e_only() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell sh
greet() {
    echo "hello"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "greet"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set -e"),
        "Expected 'set -e' for sh, got: {stdout}"
    );
    assert!(
        !stdout.contains("pipefail"),
        "sh should NOT have pipefail, got: {stdout}"
    );
}

#[test]
fn test_show_script_noerrexit_omits_set_e() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @noerrexit
greet() {
    echo "hello"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "greet"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("set -e"),
        "Expected no 'set -e' with @noerrexit, got: {stdout}"
    );
}

#[test]
fn test_polyglot_no_set_e() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell python3
greet() {
    print("hello from python")
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "greet"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("set -e"),
        "Python scripts should not get set -e, got: {stdout}"
    );
}

#[test]
fn test_or_true_pattern_works_under_errexit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
tolerant() {
    false || true
    echo "continued"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("tolerant")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "|| true pattern should work under set -e. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("continued"),
        "Expected 'continued' after || true"
    );
}

#[test]
fn test_simple_function_errexit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Simple (one-liner) functions should also get set -e
    let runfile = r#"
fail() echo "before" && false && echo "after"

wrapper() {
    fail
    echo "should not reach"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .args(["--show-script", "wrapper"])
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set -e"),
        "Simple function wrapper should have set -e, got: {stdout}"
    );
}
