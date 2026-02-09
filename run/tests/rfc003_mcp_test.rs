// RFC003: AI Agent Support & Model Context Protocol (MCP) - Tests

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
fn test_command(binary: &PathBuf) -> Command {
    let mut cmd = Command::new(binary);
    // Disable global runfile merging for test isolation
    cmd.env("RUN_NO_GLOBAL_MERGE", "1");
    cmd
}

// ========== Phase 1: Parsing @desc and @arg Attributes ==========

#[test]
fn test_parse_desc_attribute() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Restarts the docker containers and tails the logs
restart() docker compose restart
"#,
    );

    // List functions to ensure it still works with @desc
    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("restart"));
}

#[test]
fn test_parse_arg_attribute() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Scale a specific service
# @arg 1:service string The name of the docker service
# @arg 2:replicas integer The number of instances to spin up
scale() docker compose scale $1=$2
"#,
    );

    // List functions to ensure it still works with @arg
    let output = test_command(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scale"));
}

#[test]
fn test_function_with_desc_and_args_still_executable() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Greet a person
# @arg 1:name string The person's name
greet() echo "Hello, $1!"
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

// ========== Phase 2: --inspect Command ==========

#[test]
fn test_inspect_flag_exists() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Test function
test() echo "test"
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Should not fail with "unknown flag" error
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.to_lowercase().contains("unrecognized")
            && !stderr.to_lowercase().contains("unexpected")
    );
}

#[test]
fn test_inspect_output_json_structure() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Scale a specific service
# @arg 1:service string The name of the docker service
# @arg 2:replicas integer The number of instances to spin up
scale() docker compose scale $1=$2
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse as JSON
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Check structure
    assert!(json.get("tools").is_some(), "Should have 'tools' field");
    let tools = json["tools"].as_array().expect("tools should be array");
    assert_eq!(tools.len(), 1, "Should have one tool");

    let tool = &tools[0];
    assert_eq!(tool["name"].as_str().unwrap(), "scale");
    assert_eq!(
        tool["description"].as_str().unwrap(),
        "Scale a specific service"
    );

    let schema = &tool["inputSchema"];
    assert_eq!(schema["type"].as_str().unwrap(), "object");

    let properties = &schema["properties"];
    assert!(properties["service"].is_object());
    assert_eq!(properties["service"]["type"].as_str().unwrap(), "string");
    assert!(properties["replicas"].is_object());
    assert_eq!(properties["replicas"]["type"].as_str().unwrap(), "integer");

    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&serde_json::json!("service")));
    assert!(required.contains(&serde_json::json!("replicas")));
}

#[test]
fn test_inspect_function_without_metadata() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
build() echo "Building..."
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    let tools = json["tools"].as_array().unwrap();
    // Functions without @desc should not be included in MCP tool list
    assert_eq!(tools.len(), 0);
}

#[test]
fn test_inspect_multiple_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Build the project
build() echo "Building..."

# @desc Deploy to environment
# @arg 1:env string Target environment
deploy() echo "Deploying to $1"
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    let tools = json["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);

    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"build"));
    assert!(names.contains(&"deploy"));
}

#[test]
fn test_inspect_arg_with_boolean_type() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Test function with boolean
# @arg 1:verbose boolean Enable verbose output
test() echo "Verbose: $1"
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tool = &json["tools"][0];
    let properties = &tool["inputSchema"]["properties"];

    assert_eq!(properties["verbose"]["type"].as_str().unwrap(), "boolean");
}

#[test]
fn test_inspect_with_os_filtered_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @os unix
# @desc Unix-only function
unix_func() echo "Unix"

# @os windows
# @desc Windows-only function
win_func() echo "Windows"
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tools = json["tools"].as_array().unwrap();

    // On Unix, should only have unix_func
    if cfg!(unix) {
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"].as_str().unwrap(), "unix_func");
    }
}

// ========== Phase 3: MCP Server (--serve-mcp) ==========

#[test]
fn test_serve_mcp_flag_exists() {
    let binary = get_binary_path();

    // Just check that the flag is recognized (we'll kill it quickly)
    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    // Give it a moment to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Kill it
    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();

    // Should not have "unrecognized" error
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.to_lowercase().contains("unrecognized"));
}

// Note: Full MCP integration tests would require sending JSON-RPC messages
// These will be added after basic implementation is working

// ========== MCP JSON-RPC Protocol Tests ==========

#[test]
fn test_mcp_initialize_request() {
    use std::io::Write;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Test function
test() echo "test"
"#,
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    // Send initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    let stdin = child.stdin.as_mut().unwrap();
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Give it time to respond
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Kill and check output
    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain a JSON-RPC response
    assert!(
        stdout.contains("\"jsonrpc\":\"2.0\"") || stdout.contains("\"jsonrpc\": \"2.0\""),
        "Expected JSON-RPC response, got: {}",
        stdout
    );
}

#[test]
fn test_mcp_tools_list_request() {
    use std::io::Write;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Scale a service
# @arg 1:service string The service name
scale() echo "Scaling $1"
"#,
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let stdin = child.stdin.as_mut().unwrap();

    // Send initialize first
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();

    // Send tools/list request
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{}", serde_json::to_string(&list_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(500));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain tools list with our "scale" function
    assert!(
        stdout.contains("scale") || stdout.contains("\"name\""),
        "Expected tools list, got: {}",
        stdout
    );
}
