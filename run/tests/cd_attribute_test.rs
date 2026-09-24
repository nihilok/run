//! Integration tests for the `# @cd` attribute
//!
//! Tests working directory scoping, subshell isolation, polyglot support,
//! and error handling for nonexistent directories.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_raw_string_hashes)]

mod common;

use common::*;
use std::fs;

#[test]
fn test_cd_simple_function() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sub_dir = temp_dir.path().join("sub");
    fs::create_dir(&sub_dir).unwrap();

    let runfile = r#"
# @cd ./sub
pwd_simple() pwd
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .arg("pwd_simple")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute pwd_simple");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_canon = sub_dir.canonicalize().unwrap();
    assert!(
        stdout.trim().ends_with(expected_canon.to_str().unwrap())
            || stdout.trim() == expected_canon.to_str().unwrap(),
        "Expected pwd to end with {:?}, got: {}",
        expected_canon,
        stdout
    );
}

#[test]
fn test_cd_block_function() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sub_dir = temp_dir.path().join("sub_block");
    fs::create_dir(&sub_dir).unwrap();

    let runfile = r#"
# @cd ./sub_block
pwd_block() {
    pwd
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .arg("pwd_block")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute pwd_block");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_canon = sub_dir.canonicalize().unwrap();
    assert!(
        stdout.trim().ends_with(expected_canon.to_str().unwrap())
            || stdout.trim() == expected_canon.to_str().unwrap(),
        "Expected pwd to end with {:?}, got: {}",
        expected_canon,
        stdout
    );
}

#[test]
fn test_cd_subshell_isolation() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sub_dir = temp_dir.path().join("nested");
    fs::create_dir(&sub_dir).unwrap();

    let runfile = r#"
# @cd ./nested
child_task() {
    pwd
}

parent_task() {
    child_task
    pwd
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .arg("parent_task")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute parent_task");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "Expected exactly 2 lines of output, got: {stdout}"
    );

    let child_canon = sub_dir.canonicalize().unwrap();
    let parent_canon = temp_dir.path().canonicalize().unwrap();

    assert!(
        lines[0].ends_with(child_canon.to_str().unwrap())
            || lines[0] == child_canon.to_str().unwrap(),
        "Child should have run in {:?}, got: {}",
        child_canon,
        lines[0]
    );
    assert!(
        lines[1].ends_with(parent_canon.to_str().unwrap())
            || lines[1] == parent_canon.to_str().unwrap(),
        "Parent should have remained in {:?}, got: {}",
        parent_canon,
        lines[1]
    );
}

#[test]
fn test_cd_with_polyglot() {
    if !is_python_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sub_dir = temp_dir.path().join("py_sub");
    fs::create_dir(&sub_dir).unwrap();

    let runfile = r#"
# @cd ./py_sub
# @shell python
py_pwd() {
    import os
    print("PY_PWD=" + os.getcwd())
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .arg("py_pwd")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute py_pwd");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_canon = sub_dir.canonicalize().unwrap();
    let expected_str = expected_canon.to_str().unwrap();
    assert!(
        stdout.contains(&format!("PY_PWD={expected_str}")),
        "Expected Python pwd to be {:?}, got: {}",
        expected_str,
        stdout
    );
}

#[test]
fn test_cd_nonexistent_directory() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @cd ./nonexistent_sub_dir
failing_task() echo "should not execute"
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .arg("failing_task")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("directory specified in @cd does not exist"),
        "Expected error message about nonexistent directory, got stderr: {stderr}"
    );
}

#[test]
fn test_cd_structured_output() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sub_dir = temp_dir.path().join("structured_sub");
    fs::create_dir(&sub_dir).unwrap();

    let runfile = r#"
# @cd ./structured_sub
struct_task() echo "hello"
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .args(["--output-format=json", "struct_task"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute struct_task with --output-format=json");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_canon = sub_dir.canonicalize().unwrap();
    let expected_str = expected_canon.to_str().unwrap();
    assert!(
        stdout.contains(expected_str),
        "Expected JSON output to contain working_directory {:?}, got: {}",
        expected_str,
        stdout
    );
}

#[test]
fn test_cd_relative_to_sourced_file() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create pkg/inner directory
    let pkg_dir = temp_dir.path().join("pkg");
    let inner_dir = pkg_dir.join("inner");
    fs::create_dir_all(&inner_dir).unwrap();

    // Sourced file in pkg/tasks.run with @cd ./inner relative to pkg/
    let pkg_tasks = r#"
# @cd ./inner
pkg_task() pwd
"#;
    fs::write(pkg_dir.join("tasks.run"), pkg_tasks).unwrap();

    // Root Runfile sources pkg/tasks.run
    create_runfile(temp_dir.path(), "source pkg/tasks.run\n");

    let output = test_command(&binary)
        .arg("pkg_task")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute pkg_task");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_canon = inner_dir.canonicalize().unwrap();
    assert!(
        stdout.trim().ends_with(expected_canon.to_str().unwrap())
            || stdout.trim() == expected_canon.to_str().unwrap(),
        "Expected pkg_task to execute in {:?}, got: {}",
        expected_canon,
        stdout
    );
}
