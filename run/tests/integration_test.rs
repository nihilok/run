#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the compiled binary path
fn get_binary_path() -> PathBuf {
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
            .args(&["build", "--bin", "run"])
            .output()
            .expect("Failed to build binary");

        if !build_output.status.success() {
            panic!(
                "Failed to build run binary: {}",
                String::from_utf8_lossy(&build_output.stderr)
            );
        }
    }

    path
}

/// Helper to create a temporary directory for tests
fn create_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

/// Helper to create a Runfile in a directory
fn create_runfile(dir: &std::path::Path, content: &str) {
    let runfile_path = dir.join("Runfile");
    fs::write(runfile_path, content).unwrap();
}

/// Helper to create a Command with test environment
/// Sets RUN_NO_GLOBAL_MERGE to isolate tests from user's ~/.runfile
fn test_command(binary: &PathBuf) -> Command {
    let mut cmd = Command::new(binary);
    cmd.env("RUN_NO_GLOBAL_MERGE", "1");
    cmd
}

/// Helper to check if Python is available on the system
fn is_python_available() -> bool {
    which::which("python3").is_ok() || which::which("python").is_ok()
}

/// Helper to check if Node is available on the system
fn is_node_available() -> bool {
    which::which("node").is_ok()
}

/// Helper to check if Ruby is available on the system
fn is_ruby_available() -> bool {
    which::which("ruby").is_ok()
}

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn test_version_flag() {
    let binary = get_binary_path();
    let output = test_command(&binary)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Ensure the version printed matches the package version
    assert!(stdout.contains(PKG_VERSION));
}

#[test]
fn test_list_flag_no_runfile() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path()) // Override HOME to avoid loading ~/.runfile
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

    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Available functions"));  // Changed to match new format
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

    let output = test_command(&binary)
        .arg("-l")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
}

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

    let output = test_command(&binary)
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

    let output = test_command(&binary)
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
fn test_variable_default_value_syntax() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
server() echo "Starting server on port ${1:-8080}"
"#,
    );

    // Test with default value (no argument provided) - bash handles the default
    let output = test_command(&binary)
        .arg("server")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("port 8080"));

    // Test with provided value - bash substitutes the provided arg
    let output2 = test_command(&binary)
        .arg("server")
        .arg("3000")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    // The command should have been passed to bash correctly (parsing worked)
    // Bash handles the ${1:-8080} substitution at runtime
    assert!(stdout2.contains("port"));
}

#[test]
fn test_variable_default_in_flag_assignment() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // This tests the --flag=${var:-default} pattern where flag= and variable must stay together
    create_runfile(
        temp_dir.path(),
        r#"
server() echo "port=${1:-8080}"
"#,
    );

    // Test with default value
    let output = test_command(&binary)
        .arg("server")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("port=8080"),
        "Expected 'port=8080' but got: {}",
        stdout
    );

    // Test with provided value
    let output2 = test_command(&binary)
        .arg("server")
        .arg("3000")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("port=3000"),
        "Expected 'port=3000' but got: {}",
        stdout2
    );
}

#[test]
fn test_variable_default_in_unquoted_flag() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // This tests unquoted --flag=${var:-default} pattern
    create_runfile(
        temp_dir.path(),
        r#"
server() echo port=${1:-8080}
"#,
    );

    // Test with default value
    let output = test_command(&binary)
        .arg("server")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("port=8080"),
        "Expected 'port=8080' but got: {}",
        stdout
    );

    // Test with provided value
    let output2 = test_command(&binary)
        .arg("server")
        .arg("3000")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("port=3000"),
        "Expected 'port=3000' but got: {}",
        stdout2
    );
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

    let output = test_command(&binary)
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
fn test_nested_function_call() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
docker:shell() echo "Opening Docker shell for $1"
"#,
    );

    let output = test_command(&binary)
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
fn test_runfile_search_upward() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create Runfile in parent directory
    create_runfile(
        temp_dir.path(),
        r#"
parent() echo "Called from parent"
"#,
    );

    // Create a subdirectory
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let output = test_command(&binary)
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

    // Create home runfile
    let home_runfile = temp_dir.path().join(".runfile");
    fs::write(&home_runfile, "test() echo \"From home\"\n").unwrap();

    // Create local runfile in subdirectory
    let local_dir = temp_dir.path().join("project");
    fs::create_dir(&local_dir).unwrap();
    create_runfile(
        &local_dir,
        r#"
test() echo "From local"
"#,
    );

    let output = test_command(&binary)
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

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from script"));
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

    let output = test_command(&binary)
        .arg("nonexistent")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Function 'nonexistent' not found"));
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

    let output = test_command(&binary)
        .arg("test")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Check for any error message (could be parse error or function not found)
    assert!(stderr.to_lowercase().contains("error"));
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

    let output = test_command(&binary)
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
fn test_command_with_pipes() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
count() echo "one\ntwo\nthree" | wc -l
"#,
    );

    let output = test_command(&binary)
        .arg("count")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The output should contain a number (the line count)
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

    let output = test_command(&binary)
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

    let output = test_command(&binary)
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

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"));
}

#[test]
fn test_function_call_with_bare_word_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_bare_args.run");
    fs::write(
        &script_path,
        r#"
docker:logs() echo "Docker logs for: $1"
docker:logs(app)
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Docker logs for: app"));
}

#[test]
fn test_function_call_with_quoted_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_quoted_args.run");
    fs::write(
        &script_path,
        r#"
greet() echo "Hello, $1 and $2!"
greet("Alice", "Bob")
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Alice and Bob!"));
}

#[test]
fn test_function_call_mixed_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_mixed_args.run");
    fs::write(
        &script_path,
        r#"
show() echo "First: $1, Second: $2"
show(bare, "quoted")
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("First: bare, Second: quoted"));
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

    let output = test_command(&binary)
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

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("App: myapp, Env: production"));
}

#[test]
fn test_multiple_variables() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_multi_vars.run");
    fs::write(
        &script_path,
        r#"
first=Alice
second=Bob
echo "$first and $second"
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice and Bob"));
}

#[test]
fn test_variable_with_underscore() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let script_path = temp_dir.path().join("test_var_underscore.run");
    fs::write(
        &script_path,
        r#"
app_name=myapp
echo "Application: $app_name"
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Application: myapp"));
}

#[test]
fn test_generate_completion_bash() {
    let binary = get_binary_path();
    let output = test_command(&binary)
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
    let output = test_command(&binary)
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
    let output = test_command(&binary)
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

    // Set HOME to temp directory
    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it reports success
    assert!(stdout.contains("Installing zsh completion"));
    assert!(stdout.contains("Installation complete"));

    // Verify the completion file was created
    let comp_file = temp_dir.path().join(".zsh/completion/_run");
    assert!(comp_file.exists(), "Completion file should be created");

    // Verify the content is correct
    let content = fs::read_to_string(&comp_file).unwrap();
    assert!(content.contains("#compdef run"));
    assert!(content.contains("_run"));
}

#[test]
fn test_install_completion_bash() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("bash")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Installing bash completion"));
    assert!(stdout.contains("Installation complete"));

    // Verify the completion file was created
    let comp_file = temp_dir
        .path()
        .join(".local/share/bash-completion/completions/run");
    assert!(comp_file.exists(), "Bash completion file should be created");

    let content = fs::read_to_string(&comp_file).unwrap();
    assert!(content.contains("#!/usr/bin/env bash"));
    assert!(content.contains("_run_complete"));
}

#[test]
fn test_install_completion_fish() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("fish")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Installing fish completion"));
    assert!(stdout.contains("Installation complete"));

    // Verify the completion file was created
    let comp_file = temp_dir.path().join(".config/fish/completions/run.fish");
    assert!(comp_file.exists(), "Fish completion file should be created");

    let content = fs::read_to_string(&comp_file).unwrap();
    assert!(content.contains("# Fish completion script"));
    assert!(content.contains("complete -c run"));
}

#[test]
fn test_install_completion_detects_missing_zshrc_config() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create an empty .zshrc file
    let zshrc_path = temp_dir.path().join(".zshrc");
    fs::write(&zshrc_path, "# Empty zshrc\n").unwrap();

    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should suggest adding fpath and compinit
    assert!(stdout.contains("fpath=(~/.zsh/completion $fpath)"));
    assert!(stdout.contains("autoload -Uz compinit"));
}

#[test]
fn test_install_completion_detects_existing_zshrc_config() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a .zshrc file with the necessary config already present
    let zshrc_path = temp_dir.path().join(".zshrc");
    fs::write(
        &zshrc_path,
        r#"
# My zshrc
fpath=(~/.zsh/completion $fpath)
autoload -Uz compinit && compinit
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should NOT suggest adding config since it's already there
    let lines_with_echo: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("echo 'fpath=") || line.contains("echo 'autoload"))
        .collect();

    assert!(
        lines_with_echo.is_empty(),
        "Should not suggest adding config that already exists, but found: {:?}",
        lines_with_echo
    );
}

#[test]
fn test_install_completion_detects_partial_zshrc_config() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a .zshrc file with only fpath, missing compinit
    let zshrc_path = temp_dir.path().join(".zshrc");
    fs::write(
        &zshrc_path,
        r#"
# My zshrc
fpath=(~/.zsh/completion $fpath)
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should NOT suggest fpath (already present)
    assert!(
        !stdout.contains("echo 'fpath=(~/.zsh/completion $fpath)'"),
        "Should not suggest fpath since it already exists"
    );

    // But SHOULD suggest compinit (missing)
    assert!(
        stdout.contains("autoload -Uz compinit"),
        "Should suggest compinit since it's missing"
    );
}

#[test]
fn test_install_completion_auto_detect_fails_with_unknown_shell() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Set SHELL to something unsupported
    let output = test_command(&binary)
        .arg("--install-completion")
        .env("HOME", temp_dir.path())
        .env("SHELL", "/bin/ksh") // Unsupported shell
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Could not detect shell") || stderr.contains("Unsupported shell"));
    assert!(stderr.contains("bash") && stderr.contains("zsh") && stderr.contains("fish"));
}

#[test]
fn test_install_completion_overwrites_existing_file() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Pre-create the completion directory and file with old content
    let comp_dir = temp_dir.path().join(".zsh/completion");
    fs::create_dir_all(&comp_dir).unwrap();
    let comp_file = comp_dir.join("_run");
    fs::write(&comp_file, "# Old completion content\n").unwrap();

    // Install new completion
    let output = test_command(&binary)
        .arg("--install-completion")
        .arg("zsh")
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // Verify the file was overwritten with new content
    let content = fs::read_to_string(&comp_file).unwrap();
    assert!(content.contains("#compdef run"));
    assert!(!content.contains("# Old completion content"));
}

// Tests for flexible bash-like function definition syntax

#[test]
fn test_function_keyword_with_block() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet {
    echo "Hello from function keyword!"
}
"#,
    );

    let output = test_command(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from function keyword!"));
}

#[test]
fn test_function_keyword_with_parens_and_block() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet() {
    echo "Hello with parens!"
}
"#,
    );

    let output = test_command(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello with parens!"));
}

#[test]
fn test_function_keyword_inline_command() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet echo "Hello inline!"
"#,
    );

    let output = test_command(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello inline!"));
}

#[test]
fn test_function_keyword_with_parens_inline() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet() echo "Hello parens inline!"
"#,
    );

    let output = test_command(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello parens inline!"));
}

#[test]
fn test_function_keyword_namespaced() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function docker:shell {
    echo "Docker shell function"
}
"#,
    );

    let output = test_command(&binary)
        .arg("docker:shell")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Docker shell function"));
}

#[test]
fn test_function_keyword_namespaced_inline() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function docker:logs echo "Showing logs"
"#,
    );

    let output = test_command(&binary)
        .arg("docker:logs")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Showing logs"));
}

#[test]
fn test_function_keyword_with_arguments() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function greet echo "Hello, $1!"
"#,
    );

    let output = test_command(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"));
}

#[test]
fn test_function_keyword_block_multiline() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function deploy {
    echo "Step 1: Building"
    echo "Step 2: Testing"
    echo "Step 3: Deploying"
}
"#,
    );

    let output = test_command(&binary)
        .arg("deploy")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Step 1: Building"));
    assert!(stdout.contains("Step 2: Testing"));
    assert!(stdout.contains("Step 3: Deploying"));
}

#[test]
fn test_function_keyword_block_semicolon_separated() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
function quick { echo "one"; echo "two"; echo "three"; }
"#,
    );

    let output = test_command(&binary)
        .arg("quick")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("one"));
    assert!(stdout.contains("two"));
    assert!(stdout.contains("three"));
}

#[test]
fn test_mixed_function_syntaxes() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# Traditional syntax
traditional() echo "traditional"

# Function keyword with block
function keyword_block {
    echo "keyword block"
}

# Function keyword with parens
function keyword_parens() echo "keyword parens"

# Function keyword inline
function keyword_inline echo "keyword inline"
"#,
    );

    // Test all four variants
    let output1 = test_command(&binary)
        .arg("traditional")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");
    assert!(output1.status.success());
    assert!(String::from_utf8_lossy(&output1.stdout).contains("traditional"));

    let output2 = test_command(&binary)
        .arg("keyword_block")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");
    assert!(output2.status.success());
    assert!(String::from_utf8_lossy(&output2.stdout).contains("keyword block"));

    let output3 = test_command(&binary)
        .arg("keyword_parens")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");
    assert!(output3.status.success());
    assert!(String::from_utf8_lossy(&output3.stdout).contains("keyword parens"));

    let output4 = test_command(&binary)
        .arg("keyword_inline")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");
    assert!(output4.status.success());
    assert!(String::from_utf8_lossy(&output4.stdout).contains("keyword inline"));
}

// Tests for RFC 001: Attribute Comments (Platform Guards & Interpreter Selection)

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

    let output = test_command(&binary)
        .arg("clean")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // On Unix (Linux/macOS), only the unix variant should be available
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

    let output = test_command(&binary)
        .arg("clean")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // On Unix, the windows-only function should not be found
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

    let output = test_command(&binary)
        .arg("build")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // On Linux, only the Linux variant should be available
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

    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should always show the function without attributes
    assert!(stdout.contains("no_attr_func"));

    // On Unix, should show unix_func but not windows_func
    if cfg!(unix) {
        assert!(stdout.contains("unix_func"));
        assert!(!stdout.contains("windows_func"));
    }
}

#[test]
fn test_shell_attribute_python() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
math() {
    import sys
    result = 10 + 20
    print(f"Result: {result}")
}
"#,
    );

    // Check if python is available
    if is_python_available() {
        let output = test_command(&binary)
            .arg("math")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Result: 30"));
    }
}

#[test]
fn test_shell_attribute_python_with_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
calc() {
    import sys
    if len(sys.argv) > 1:
        print(f"Argument: {sys.argv[1]}")
    else:
        print("No arguments")
}
"#,
    );

    // Check if python is available
    if is_python_available() {
        let output = test_command(&binary)
            .arg("calc")
            .arg("hello")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Argument: hello"));
    }
}

#[test]
fn test_shell_attribute_node() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell node
server() {
    console.log("Server starting...");
    console.log("Port: 3000");
}
"#,
    );

    // Check if node is available
    if is_node_available() {
        let output = test_command(&binary)
            .arg("server")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Server starting..."));
        assert!(stdout.contains("Port: 3000"));
    }
}

#[test]
fn test_shell_attribute_node_with_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell node
greet() {
    const args = process.argv.slice(1);
    if (args.length > 0) {
        console.log(`Hello, ${args[0]}!`);
    } else {
        console.log("Hello, World!");
    }
}
"#,
    );

    // Check if node is available
    if is_node_available() {
        let output = test_command(&binary)
            .arg("greet")
            .arg("Alice")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello, Alice!"));
    }
}

#[test]
fn test_shell_attribute_bash_explicit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell bash
test_func() echo "Running in bash"
"#,
    );

    // Bash should be available on Unix systems
    if cfg!(unix) && which::which("bash").is_ok() {
        let output = test_command(&binary)
            .arg("test_func")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Running in bash"));
    }
}

#[test]
fn test_shell_attribute_ruby() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell ruby
hello() {
    puts "Hello from Ruby!"
    puts "Ruby version: #{RUBY_VERSION}"
}
"#,
    );

    // Check if ruby is available
    if is_ruby_available() {
        let output = test_command(&binary)
            .arg("hello")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Ruby!"));
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
unix_python() {
    print("Unix Python function")
}
"#,
    );

    // Only run on Unix with Python available
    if cfg!(unix) && (is_python_available()) {
        let output = test_command(&binary)
            .arg("unix_python")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Unix Python function"));
    }
}

#[test]
fn test_attribute_comment_not_confused_with_regular_comment() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# This is a regular comment
# @os unix
clean() echo "Unix clean"

# Another regular comment
build() echo "Regular build"
"#,
    );

    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Both functions should be listed (build always, clean on unix)
    assert!(stdout.contains("build"));
    if cfg!(unix) {
        assert!(stdout.contains("clean"));
    }
}

#[test]
fn test_multiple_attributes_on_same_function() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os linux
# @os macos
posix_func() echo "POSIX function"
"#,
    );

    let output = test_command(&binary)
        .arg("posix_func")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Should work on both Linux and macOS
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("POSIX function"));
    }
}

#[test]
fn test_simple_function_with_shell_attribute() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
simple() print("Simple inline Python")
"#,
    );

    // Check if python is available
    if is_python_available() {
        let output = test_command(&binary)
            .arg("simple")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Simple inline Python"));
    }
}

#[test]
fn test_inline_block_with_semicolons() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Test traditional () syntax with inline block containing semicolons
    create_runfile(
        temp_dir.path(),
        r#"
test_inline() { echo "a"; echo "b"; echo "c" }
"#,
    );

    let output = test_command(&binary)
        .arg("test_inline")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
    assert!(stdout.contains("c"));
}

#[test]
fn test_inline_block_with_trailing_semicolon() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Test with trailing semicolon before closing brace
    create_runfile(
        temp_dir.path(),
        r#"
test_trailing() { echo "x"; echo "y"; echo "z"; }
"#,
    );

    let output = test_command(&binary)
        .arg("test_trailing")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("x"));
    assert!(stdout.contains("y"));
    assert!(stdout.contains("z"));
}

#[test]
fn test_function_keyword_inline_block_with_semicolons() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Test function keyword syntax with inline block
    create_runfile(
        temp_dir.path(),
        r#"
function test_func { echo "first"; echo "second"; echo "third" }
"#,
    );

    let output = test_command(&binary)
        .arg("test_func")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("first"));
    assert!(stdout.contains("second"));
    assert!(stdout.contains("third"));
}

#[test]
fn test_shell_node_multiline_with_loop() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Test Node.js with multi-line code containing a for loop
    // This ensures newlines are preserved when passed to node -e
    create_runfile(
        temp_dir.path(),
        r#"
# @shell node
counter() {
    console.log("Starting count");
    for (let i = 0; i < 3; i++) {
        console.log(`Count: ${i}`);
    }
    console.log("Done");
}
"#,
    );

    if is_node_available() {
        let output = test_command(&binary)
            .arg("counter")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Starting count"));
        assert!(stdout.contains("Count: 0"));
        assert!(stdout.contains("Count: 1"));
        assert!(stdout.contains("Count: 2"));
        assert!(stdout.contains("Done"));
    }
}

#[test]
fn test_shell_python_multiline_with_loop() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Test Python with multi-line code containing a for loop
    // This ensures indentation is preserved for Python
    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
counter() {
    print("Starting count")
    for i in range(3):
        print(f"Count: {i}")
    print("Done")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("counter")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Starting count"));
        assert!(stdout.contains("Count: 0"));
        assert!(stdout.contains("Count: 1"));
        assert!(stdout.contains("Count: 2"));
        assert!(stdout.contains("Done"));
    }
}

// ========== Shebang Detection Tests (RFC002) ==========

#[test]
fn test_shebang_python_basic() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
analyze() {
    #!/usr/bin/env python
    import sys
    print("Hello from Python via shebang!")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("analyze")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Python via shebang!"));
    }
}

#[test]
fn test_shebang_python3_explicit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
calc() {
    #!/usr/bin/env python3
    import math
    print(f"Pi is {math.pi:.2f}")
}
"#,
    );

    if which::which("python3").is_ok() {
        let output = test_command(&binary)
            .arg("calc")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Pi is 3.14"));
    }
}

#[test]
fn test_shebang_node() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
server() {
    #!/usr/bin/env node
    console.log("Node server via shebang");
    console.log("Port: 3000");
}
"#,
    );

    if is_node_available() {
        let output = test_command(&binary)
            .arg("server")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Node server via shebang"));
        assert!(stdout.contains("Port: 3000"));
    }
}

#[test]
fn test_shebang_bash() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
setup() {
    #!/usr/bin/env bash
    echo "Setting up with bash shebang"
    echo "Configuration complete"
}
"#,
    );

    if cfg!(unix) && which::which("bash").is_ok() {
        let output = test_command(&binary)
            .arg("setup")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Setting up with bash shebang"));
        assert!(stdout.contains("Configuration complete"));
    }
}

#[test]
fn test_shebang_direct_path_bash() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
test_func() {
    #!/bin/bash
    echo "Using direct bash path"
}
"#,
    );

    if cfg!(unix) && which::which("bash").is_ok() {
        let output = test_command(&binary)
            .arg("test_func")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Using direct bash path"));
    }
}

#[test]
fn test_shebang_direct_path_sh() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
simple() {
    #!/bin/sh
    echo "Using sh"
}
"#,
    );

    if cfg!(unix) {
        let output = test_command(&binary)
            .arg("simple")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Using sh"));
    }
}

#[test]
fn test_shebang_with_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
process() {
    #!/usr/bin/env python
    import sys
    if len(sys.argv) > 1:
        print(f"Processing: {sys.argv[1]}")
    else:
        print("No args")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("process")
            .arg("data.json")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Processing: data.json"));
    }
}

#[test]
fn test_shebang_precedence_attribute_wins() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // @shell attribute should take precedence over shebang
    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
test_precedence() {
    #!/usr/bin/env node
    # This should execute with Python, not Node
    import sys
    print("Python wins!")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("test_precedence")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Python wins!"));
        // Should NOT contain Node.js error messages
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("SyntaxError"));
    }
}

#[test]
fn test_shebang_with_comment_before() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
analyze() {
    # This is a comment
    #!/usr/bin/env python
    import sys
    print("Shebang after comment")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("analyze")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Shebang after comment"));
    }
}

#[test]
fn test_shebang_multiline_code() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
counter() {
    #!/usr/bin/env python
    print("Counting")
    for i in range(3):
        print(f"Number: {i}")
    print("Done")
}
"#,
    );

    if is_python_available() {
        let output = test_command(&binary)
            .arg("counter")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Counting"));
        assert!(stdout.contains("Number: 0"));
        assert!(stdout.contains("Number: 1"));
        assert!(stdout.contains("Number: 2"));
        assert!(stdout.contains("Done"));
    }
}

#[test]
fn test_shebang_ruby() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
greet() {
    #!/usr/bin/env ruby
    puts "Hello from Ruby via shebang!"
}
"#,
    );

    if is_ruby_available() {
        let output = test_command(&binary)
            .arg("greet")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Ruby via shebang!"));
    }
}

#[test]
fn test_no_shebang_uses_default_shell() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
simple() {
    echo "No shebang, using default shell"
}
"#,
    );

    let output = test_command(&binary)
        .arg("simple")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No shebang, using default shell"));
}

#[test]
fn test_shebang_not_first_line_ignored() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Shebang not on first non-empty line should be ignored
    create_runfile(
        temp_dir.path(),
        r#"
broken() {
    echo "First line"
    #!/usr/bin/env python
    echo "Should use default shell, not Python"
}
"#,
    );

    let output = test_command(&binary)
        .arg("broken")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("First line"));
    assert!(stdout.contains("Should use default shell, not Python"));
}

// ========== Tests for --runfile Feature ==========

#[test]
fn test_runfile_flag_with_file_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a Runfile in a non-standard location
    let custom_runfile = temp_dir.path().join("CustomRunfile");
    fs::write(
        &custom_runfile,
        r#"
test_function() {
    echo "Hello from custom Runfile"
}
"#,
    )
    .unwrap();

    // Run with --runfile pointing to the file
    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&custom_runfile)
        .arg("test_function")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from custom Runfile"));
}

#[test]
fn test_runfile_flag_with_directory_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a subdirectory with a Runfile
    let subdir = temp_dir.path().join("project");
    fs::create_dir(&subdir).unwrap();

    create_runfile(
        &subdir,
        r#"
greet() {
    echo "Hello from subdirectory Runfile"
}
"#,
    );

    // Run from a different directory, pointing --runfile to the subdirectory
    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&subdir)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from subdirectory Runfile"));
}

#[test]
fn test_runfile_flag_overrides_current_directory() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a Runfile in the current directory
    create_runfile(
        temp_dir.path(),
        r#"
main() {
    echo "From current directory"
}
"#,
    );

    // Create a different Runfile in a subdirectory
    let subdir = temp_dir.path().join("other");
    fs::create_dir(&subdir).unwrap();
    create_runfile(
        &subdir,
        r#"
main() {
    echo "From other directory"
}
"#,
    );

    // Run from temp_dir but point --runfile to subdir
    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&subdir)
        .arg("main")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("From other directory"));
    assert!(!stdout.contains("From current directory"));
}

#[test]
fn test_runfile_flag_with_list() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let custom_runfile = temp_dir.path().join("MyRunfile");
    fs::write(
        &custom_runfile,
        r#"
build() echo "building"
test() echo "testing"
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&custom_runfile)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build"));
    assert!(stdout.contains("test"));
}

#[test]
fn test_runfile_flag_nonexistent_file() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let nonexistent = temp_dir.path().join("DoesNotExist");

    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&nonexistent)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No Runfile found"));
}

#[test]
fn test_runfile_flag_with_relative_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a subdirectory with a Runfile
    let subdir = temp_dir.path().join("configs");
    fs::create_dir(&subdir).unwrap();

    let runfile_path = subdir.join("project.runfile");
    fs::write(
        &runfile_path,
        r#"
relative_test() {
    echo "Loaded via relative path"
}
"#,
    )
    .unwrap();

    // Run with a relative path
    let output = test_command(&binary)
        .arg("--runfile")
        .arg("configs/project.runfile")
        .arg("relative_test")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Loaded via relative path"));
}

#[test]
fn test_runfile_flag_with_inspect() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let custom_runfile = temp_dir.path().join("TestRunfile");
    fs::write(
        &custom_runfile,
        r#"
# @desc Test function for inspection
# @arg 1:name string The name to greet
greet() {
    echo "Hello, $1"
}
"#,
    )
    .unwrap();

    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&custom_runfile)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON to verify structure
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert!(json["tools"].is_array());
    let tools = json["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"].as_str().unwrap(), "greet");
    assert_eq!(
        tools[0]["description"].as_str().unwrap(),
        "Test function for inspection"
    );
}

#[test]
fn test_runfile_flag_with_nested_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let custom_runfile = temp_dir.path().join("NestedRunfile");
    fs::write(
        &custom_runfile,
        r#"
docker:up() {
    echo "Starting containers"
}

docker:down() {
    echo "Stopping containers"
}
"#,
    )
    .unwrap();

    // Test first nested function
    let output1 = test_command(&binary)
        .arg("--runfile")
        .arg(&custom_runfile)
        .arg("docker:up")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output1.status.success());
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(stdout1.contains("Starting containers"));

    // Test second nested function
    let output2 = test_command(&binary)
        .arg("--runfile")
        .arg(&custom_runfile)
        .arg("docker:down")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("Stopping containers"));
}

#[test]
fn test_runfile_flag_with_absolute_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let custom_runfile = temp_dir.path().join("AbsoluteRunfile");
    fs::write(
        &custom_runfile,
        r#"
absolute_test() {
    echo "Using absolute path"
}
"#,
    )
    .unwrap();

    // Use the absolute path
    let absolute_path = custom_runfile.canonicalize().unwrap();

    let output = test_command(&binary)
        .arg("--runfile")
        .arg(&absolute_path)
        .arg("absolute_test")
        .current_dir("/tmp") // Run from a different directory
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Using absolute path"));
}
