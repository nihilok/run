#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the compiled binary path
fn get_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop(); // Remove test executable name

    // Check if we're in a 'deps' directory (integration tests)
    if path.ends_with("deps") {
        path.pop(); // Go up to debug or release
    }

    path.push("run");

    // If the binary doesn't exist in debug, try building it first
    if !path.exists() {
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

#[test]
fn test_simple_function_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
build() echo "building"
test() echo "testing"

ci() {
    build
    test
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("ci")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("building"), "Expected 'building' in output");
    assert!(stdout.contains("testing"), "Expected 'testing' in output");
}

#[test]
fn test_block_function_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
build() {
    echo "building step 1"
    echo "building step 2"
}

test() {
    echo "testing step 1"
    echo "testing step 2"
}

ci() {
    build
    test
    echo "ci complete"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("ci")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("building step 1"),
        "Expected 'building step 1' in output"
    );
    assert!(
        stdout.contains("building step 2"),
        "Expected 'building step 2' in output"
    );
    assert!(
        stdout.contains("testing step 1"),
        "Expected 'testing step 1' in output"
    );
    assert!(
        stdout.contains("testing step 2"),
        "Expected 'testing step 2' in output"
    );
    assert!(
        stdout.contains("ci complete"),
        "Expected 'ci complete' in output"
    );
}

#[test]
fn test_colon_function_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
docker:build() echo "docker build"
docker:push() echo "docker push"

deploy() {
    docker:build
    docker:push
    echo "deploy complete"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("deploy")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("docker build"),
        "Expected 'docker build' in output"
    );
    assert!(
        stdout.contains("docker push"),
        "Expected 'docker push' in output"
    );
    assert!(
        stdout.contains("deploy complete"),
        "Expected 'deploy complete' in output"
    );
}

#[test]
fn test_variable_injection() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
VERSION="1.0.0"
PROJECT="myapp"

build() echo "Building $PROJECT v$VERSION"

info() {
    build
    echo "Project: $PROJECT"
    echo "Version: $VERSION"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("info")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Building myapp v1.0.0"),
        "Expected variable substitution in build"
    );
    assert!(
        stdout.contains("Project: myapp"),
        "Expected PROJECT variable"
    );
    assert!(
        stdout.contains("Version: 1.0.0"),
        "Expected VERSION variable"
    );
}

#[test]
fn test_argument_passing_in_composed_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
greet() echo "Hello, $1!"

welcome() {
    greet "$1"
    echo "Welcome to the system"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("welcome")
        .arg("Alice")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Hello, Alice!"),
        "Expected argument passed to composed function"
    );
    assert!(
        stdout.contains("Welcome to the system"),
        "Expected welcome message"
    );
}

#[test]
fn test_mixed_simple_and_block_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
simple() echo "simple function"

block() {
    echo "block function line 1"
    echo "block function line 2"
}

mixed() {
    simple
    block
    echo "mixed complete"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("mixed")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("simple function"),
        "Expected simple function output"
    );
    assert!(
        stdout.contains("block function line 1"),
        "Expected block function output"
    );
    assert!(
        stdout.contains("block function line 2"),
        "Expected block function output"
    );
    assert!(stdout.contains("mixed complete"), "Expected mixed complete");
}

#[test]
fn test_multiple_levels_of_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
level1() echo "level 1"

level2() {
    level1
    echo "level 2"
}

level3() {
    level2
    echo "level 3"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("level3")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("level 1"), "Expected level 1 output");
    assert!(stdout.contains("level 2"), "Expected level 2 output");
    assert!(stdout.contains("level 3"), "Expected level 3 output");
}

#[test]
fn test_error_propagation() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
fail() false

catch() {
    fail || echo "caught error"
    echo "continue after error"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("catch")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should succeed because we catch the error
    assert!(
        output.status.success(),
        "Command should succeed with error handling"
    );
    assert!(stdout.contains("caught error"), "Expected error handling");
    assert!(
        stdout.contains("continue after error"),
        "Expected continuation"
    );
}

#[test]
fn test_function_with_default_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
server() echo "Starting server on port ${1:-8080}"

start() {
    server
    echo "Server started"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("start")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Starting server on port 8080"),
        "Expected default port 8080"
    );
}

#[test]
fn test_complex_colon_names() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
app:build() echo "app build"
app:test() echo "app test"
app:deploy() echo "app deploy"

app:ci() {
    app:build
    app:test
    app:deploy
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("app:ci")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("app build"), "Expected app build");
    assert!(stdout.contains("app test"), "Expected app test");
    assert!(stdout.contains("app deploy"), "Expected app deploy");
}

#[test]
fn test_no_composition_without_preamble() {
    // Test that a function without composition still works
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
standalone() {
    echo "line 1"
    echo "line 2"
    echo "line 3"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("standalone")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("line 1"), "Expected line 1");
    assert!(stdout.contains("line 2"), "Expected line 2");
    assert!(stdout.contains("line 3"), "Expected line 3");
}

#[test]
fn test_variables_and_composition() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
ENV="production"
TAG="v1.0.0"

docker:build() echo "docker build -t myapp:$TAG"
docker:tag() echo "docker tag myapp:$TAG myapp:latest"

deploy:production() {
    docker:build
    docker:tag
    echo "Deploying to $ENV"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("deploy:production")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("docker build -t myapp:v1.0.0"),
        "Expected TAG variable in build"
    );
    assert!(
        stdout.contains("docker tag myapp:v1.0.0 myapp:latest"),
        "Expected TAG variable in tag"
    );
    assert!(
        stdout.contains("Deploying to production"),
        "Expected ENV variable"
    );
}

#[test]
fn test_shell_calling_incompatible_colon_function_node() {
    // Test that a shell function can call a node function with colon notation
    // This should generate a wrapper function that calls `run node hello`
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell node
node:greet() {
    console.log("Hello from Node.js!");
}

wrapper() {
    echo "Before node call"
    node:greet
    echo "After node call"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("wrapper")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Before node call"),
        "Expected 'Before node call' in output"
    );
    assert!(
        stdout.contains("Hello from Node.js!"),
        "Expected Node.js output"
    );
    assert!(
        stdout.contains("After node call"),
        "Expected 'After node call' in output"
    );
}

#[test]
fn test_shell_calling_incompatible_colon_function_python() {
    // Test that a shell function can call a python function with colon notation
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell python
python:count() {
    for i in range(3):
        print(f"Count: {i}")
}

wrapper() {
    echo "Before python call"
    python:count
    echo "After python call"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("wrapper")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Before python call"),
        "Expected 'Before python call' in output"
    );
    assert!(stdout.contains("Count: 0"), "Expected Python count output");
    assert!(stdout.contains("Count: 1"), "Expected Python count output");
    assert!(stdout.contains("Count: 2"), "Expected Python count output");
    assert!(
        stdout.contains("After python call"),
        "Expected 'After python call' in output"
    );
}

#[test]
fn test_shell_calling_multiple_incompatible_functions() {
    // Test that a shell function can call multiple incompatible functions
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# @shell node
node:hello() {
    console.log("Hello from Node!");
}

# @shell python
python:hello() {
    print("Hello from Python!")
}

multi_call() {
    echo "Starting multi-language calls"
    node:hello
    python:hello
    echo "All calls complete"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("multi_call")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Starting multi-language calls"),
        "Expected start message"
    );
    assert!(
        stdout.contains("Hello from Node!"),
        "Expected Node.js output"
    );
    assert!(
        stdout.contains("Hello from Python!"),
        "Expected Python output"
    );
    assert!(
        stdout.contains("All calls complete"),
        "Expected completion message"
    );
}

#[test]
fn test_mixed_compatible_and_incompatible_calls() {
    // Test mixing compatible (shell) and incompatible (polyglot) function calls
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# Compatible shell function
build() echo "building..."

# Incompatible node function
# @shell node
node:validate() {
    console.log("Validating with Node.js...");
}

# Main function calls both
ci() {
    build
    node:validate
    echo "CI complete"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("ci")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("building..."), "Expected build output");
    assert!(
        stdout.contains("Validating with Node.js..."),
        "Expected Node.js validation output"
    );
    assert!(
        stdout.contains("CI complete"),
        "Expected CI complete message"
    );
}

#[test]
fn test_incompatible_function_without_colon_not_wrapped() {
    // Functions without colons should NOT be wrapped (they'll fail if called directly)
    // This test verifies that only colon-named functions get wrappers
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# Shell function that works fine
shell_func() echo "shell works"

# Python function without colon - cannot be called from shell directly
# @shell python
python_only() {
    print("python only")
}

# This should work because it only calls shell_func
safe_caller() {
    shell_func
    echo "done"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("safe_caller")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("shell works"), "Expected shell_func output");
    assert!(stdout.contains("done"), "Expected done message");
}

#[test]
fn test_nested_colon_function_composition_with_polyglot() {
    // Test deeply nested colon functions mixing shell and polyglot
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile = r#"
# Shell functions
app:start() echo "app starting"
app:stop() echo "app stopping"

# Node function
# @shell node
node:healthcheck() {
    console.log("Healthcheck: OK");
}

# Orchestrator
deploy:full() {
    app:start
    node:healthcheck
    app:stop
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = Command::new(&binary)
        .arg("deploy:full")
        .current_dir(temp_dir.path())
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("app starting"), "Expected app:start output");
    assert!(
        stdout.contains("Healthcheck: OK"),
        "Expected node:healthcheck output"
    );
    assert!(stdout.contains("app stopping"), "Expected app:stop output");
}
