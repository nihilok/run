//! Basic function execution tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::fs;
use std::process::Command;

#[test]
fn test_simple_function_call() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet() echo "Hello from run!"
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from run!"));
}

#[test]
fn test_function_with_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet() echo "Hello, $1!"
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
fn test_function_with_multiple_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
add() echo "$1 + $2 = $(($1 + $2))"
"#,
    );

    let output = Command::new(&binary)
        .arg("add")
        .arg("5")
        .arg("3")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("5 + 3 = 8"));
}

#[test]
fn test_all_args_substitution() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
echo_all() echo "All args: $@"
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
    assert!(stdout.contains("All args: foo bar baz"));
}

#[test]
fn test_nested_function_call() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
docker:shell() echo "Opening Docker shell for $1"
"#,
    );

    let output = Command::new(&binary)
        .arg("docker")
        .arg("shell")
        .arg("myapp")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Opening Docker shell for myapp"));
}

#[test]
fn test_function_not_found() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
build() echo "Building..."
"#,
    );

    let output = Command::new(&binary)
        .arg("nonexistent")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Function 'nonexistent' not found"));
}

#[test]
fn test_command_with_pipes() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
count() echo "one\ntwo\nthree" | wc -l
"#,
    );

    let output = Command::new(&binary)
        .arg("count")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().parse::<i32>().is_ok());
}

#[test]
fn test_comment_handling() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# This is a comment
test() echo "Testing"
# Another comment
"#,
    );

    let output = Command::new(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test"));
}

#[test]
fn test_escaped_newlines() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
multiline() echo "This is a" \
    "multi-line" \
    "command"
"#,
    );

    let output = Command::new(&binary)
        .arg("multiline")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("This is a multi-line command"));
}

#[test]
fn test_function_call_with_parentheses() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_parens.run");
    fs::write(
        &script_path,
        r#"
greet() echo "Hello, $1!"
greet(World)
"#,
    )
    .unwrap();

    let output = Command::new(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"));
}

#[test]
fn test_runfile_search_upward() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
parent() echo "Called from parent"
"#,
    );

    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let output = Command::new(&binary)
        .arg("parent")
        .current_dir(&subdir)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Called from parent"));
}

#[test]
fn test_local_runfile_precedence() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let home_runfile = temp_dir.path().join(".runfile");
    fs::write(&home_runfile, "test() echo \"From home\"\n").unwrap();

    let local_dir = temp_dir.path().join("project");
    fs::create_dir(&local_dir).unwrap();
    create_runfile(
        &local_dir,
        r#"
test() echo "From local"
"#,
    );

    let output = Command::new(&binary)
        .arg("test")
        .current_dir(&local_dir)
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("From local"));
    assert!(!stdout.contains("From home"));
}

#[test]
fn test_execute_script_file() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test.run");
    fs::write(
        &script_path,
        r#"
hello() echo "Hello from script"
hello()
"#,
    )
    .unwrap();

    let output = Command::new(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from script"));
}

#[test]
fn test_parse_error_handling() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
invalid syntax here
"#,
    );

    let output = Command::new(&binary)
        .arg("test")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.to_lowercase().contains("error"));
}

#[test]
fn test_variable_assignment_and_usage() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_vars.run");
    fs::write(
        &script_path,
        r#"
name=World
echo "Hello, $name!"
"#,
    )
    .unwrap();

    let output = Command::new(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"));
}

#[test]
fn test_variable_in_function_template() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_var_function.run");
    fs::write(
        &script_path,
        r#"
app_name=myapp
show() echo "App: $app_name, Env: $1"
show(production)
"#,
    )
    .unwrap();

    let output = Command::new(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("App: myapp, Env: production"));
}

#[test]
fn test_variable_default_value_syntax() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
server() echo "Starting server on port ${1:-8080}"
"#,
    );

    let output = Command::new(&binary)
        .arg("server")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("port 8080"));

    let output2 = Command::new(&binary)
        .arg("server")
        .arg("3000")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("port"));
}
