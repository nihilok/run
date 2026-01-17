//! Attribute tests (@os, @shell, @desc, @arg)

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::process::Command;

#[test]
fn test_os_attribute_unix_on_unix() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os unix
clean() echo "Unix clean"

# @os windows
clean() echo "Windows clean"
"#,
    );

    let output = Command::new(&binary)
        .arg("clean")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    if cfg!(unix) {
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Unix clean"));
        assert!(!stdout.contains("Windows clean"));
    }
}

#[test]
fn test_os_attribute_windows_on_unix() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os windows
clean() echo "Windows only"
"#,
    );

    let output = Command::new(&binary)
        .arg("clean")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    if cfg!(unix) {
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Function 'clean' not found"));
    }
}

#[test]
fn test_os_attribute_linux() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os linux
build() echo "Linux build"

# @os macos
build() echo "macOS build"
"#,
    );

    let output = Command::new(&binary)
        .arg("build")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    if cfg!(target_os = "linux") {
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Linux build"));
    }
}

#[test]
fn test_os_attribute_list_shows_platform_specific() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os unix
unix_func() echo "Unix function"

# @os windows
windows_func() echo "Windows function"

no_attr_func() echo "No attribute"
"#,
    );

    let output = Command::new(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("no_attr_func"));

    if cfg!(unix) {
        assert!(stdout.contains("unix_func"));
        assert!(!stdout.contains("windows_func"));
    }
}

#[test]
fn test_combined_os_and_shell_attributes() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os unix
# @shell python
py_unix() {
    print("Python on Unix")
}
"#,
    );

    if cfg!(unix) && is_python_available() {
        let output = Command::new(&binary)
            .arg("py_unix")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Python on Unix"));
    }
}

#[test]
fn test_simple_function_with_shell_attribute() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell bash
test_bash() echo "Hello from bash"
"#,
    );

    let output = Command::new(&binary)
        .arg("test_bash")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from bash"));
}
