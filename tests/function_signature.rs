//! Integration tests for RFC004 function signature notation

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::process::Command;

#[test]
fn test_simple_params_named_substitution() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet(name) echo "Hello, $name!"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("Alice")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Alice!"));
}

#[test]
fn test_params_with_defaults() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet(name = "World") echo "Hello, $name!"
"#,
    );

    // Test with argument
    let output = Command::new(&binary)
        .arg("greet")
        .arg("Alice")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Alice!"));

    // Test without argument (should use default)
    let output = Command::new(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"));
}

#[test]
fn test_multiple_params_with_mixed_defaults() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
deploy(env, version = "latest") echo "Deploying $version to $env"
"#,
    );

    // Test with both arguments
    let output = Command::new(&binary)
        .arg("deploy")
        .arg("production")
        .arg("v1.2.3")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Deploying v1.2.3 to production"));

    // Test with only required argument
    let output = Command::new(&binary)
        .arg("deploy")
        .arg("staging")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Deploying latest to staging"));
}

#[test]
fn test_rest_parameter() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
echo_all(...args) echo "Args: $args"
"#,
    );

    let output = Command::new(&binary)
        .arg("echo_all")
        .arg("foo")
        .arg("bar")
        .arg("baz")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Args: foo bar baz"));
}

#[test]
fn test_mixed_params_and_rest() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
run_command(container, ...command) echo "Running in $container: $command"
"#,
    );

    let output = Command::new(&binary)
        .arg("run_command")
        .arg("my-app")
        .arg("ls")
        .arg("-la")
        .arg("/app")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running in my-app: ls -la /app"));
}

#[test]
fn test_block_function_with_params() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
deploy(env, version) {
    echo "Environment: $env"
    echo "Version: $version"
}
"#,
    );

    let output = Command::new(&binary)
        .arg("deploy")
        .arg("production")
        .arg("v2.0.0")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Environment: production"));
    assert!(stdout.contains("Version: v2.0.0"));
}

#[test]
fn test_backward_compatibility_positional() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# Old style without params - should still work
greet() echo "Hello, $1!"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("Bob")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Bob!"));
}

#[test]
fn test_params_with_positional_fallback() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# New style with params but also using $1 should work
greet(name) echo "Hello, $name (also $1)!"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("Charlie")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Charlie (also Charlie)!"));
}

#[test]
fn test_function_keyword_with_params() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet(name) echo "Hello, $name!"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("Dave")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Dave!"));
}

#[test]
fn test_quoted_default_value() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet(greeting = "Hello, World") echo "$greeting"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World"));
}
