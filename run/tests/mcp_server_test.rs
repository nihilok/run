// RFC003: AI Agent Support & Model Context Protocol (MCP) - Tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get the compiled binary path
fn get_binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_run"))
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
        r"
# @desc Restarts the docker containers and tails the logs
restart() docker compose restart
",
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
        r"
# @desc Scale a specific service
# @arg 1:service string The name of the docker service
# @arg 2:replicas integer The number of instances to spin up
scale() docker compose scale $1=$2
",
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
        r"
# @desc Scale a specific service
# @arg 1:service string The name of the docker service
# @arg 2:replicas integer The number of instances to spin up
scale() docker compose scale $1=$2
",
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
        "Expected JSON-RPC response, got: {stdout}"
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
        "Expected tools list, got: {stdout}"
    );
}
// ========== MCP tools/call Tests ==========

#[test]
fn test_mcp_tools_call_executes_function() {
    use std::io::Write;
    use std::time::Duration;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Greet the user
# @arg name str Name to greet
greet(name: str) echo "Hello, $name!"
"#,
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");

    let stdin = child.stdin.as_mut().unwrap();

    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "greet",
            "arguments": { "name": "World" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    std::thread::sleep(Duration::from_secs(1));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Hello, World!"),
        "Expected 'Hello, World!' in output, got: {stdout}"
    );
    assert!(
        stdout.contains("\"isError\":false") || stdout.contains("\"isError\": false"),
        "Expected isError=false, got: {stdout}"
    );
}

#[test]
fn test_mcp_tools_call_non_zero_exit_sets_is_error() {
    use std::io::Write;
    use std::time::Duration;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Always fails
fail() {
    echo "About to fail"
    exit 1
}
"#,
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");

    let stdin = child.stdin.as_mut().unwrap();

    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "fail",
            "arguments": {}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    std::thread::sleep(Duration::from_secs(1));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("\"isError\":true") || stdout.contains("\"isError\": true"),
        "Expected isError=true for failing tool, got: {stdout}"
    );
}

#[test]
fn test_mcp_tools_call_unknown_tool_returns_error() {
    use std::io::Write;
    use std::time::Duration;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Say hello
hello() echo "hello"
"#,
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");

    let stdin = child.stdin.as_mut().unwrap();

    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool_xyz",
            "arguments": {}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    std::thread::sleep(Duration::from_millis(500));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should get a JSON-RPC error response (not isError in result, but error at protocol level)
    assert!(
        stdout.contains("\"error\""),
        "Expected JSON-RPC error for unknown tool, got: {stdout}"
    );
    assert!(
        stdout.contains("Tool not found") || stdout.contains("-32602"),
        "Expected Tool not found error, got: {stdout}"
    );
}

// ========== Tests for Global + Project Runfile Merging ==========

#[test]
#[serial]
fn test_merged_global_project_list() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a global runfile
    let home_dir = std::env::var("HOME").unwrap();
    let global_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.test_merge");
    fs::write(
        &global_runfile,
        r#"
# @desc Global utility function
global_util() {
    echo "From global"
}

# @desc Shared function (will be overridden)
shared() {
    echo "Global version"
}
"#,
    )
    .unwrap();

    // Create a project runfile
    create_runfile(
        temp_dir.path(),
        r#"
# @desc Project-specific function
project_func() {
    echo "From project"
}

# @desc Shared function (overrides global)
shared() {
    echo "Project version"
}
"#,
    );

    // Temporarily swap global runfile (use unique backup name to avoid test interference)
    let original_runfile = std::path::PathBuf::from(&home_dir).join(".runfile");
    let backup_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.backup_test_merge");
    if original_runfile.exists() {
        fs::rename(&original_runfile, &backup_runfile).ok();
    }
    fs::rename(&global_runfile, &original_runfile).unwrap();

    let output = Command::new(&binary)
        .arg("--list")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Restore original runfile
    if original_runfile.exists() {
        fs::remove_file(&original_runfile).ok();
    }
    if backup_runfile.exists() {
        fs::rename(&backup_runfile, &original_runfile).ok();
    }

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show both sources
    assert!(stdout.contains("From ./Runfile:"));
    assert!(stdout.contains("From ~/.runfile:"));

    // Should show project functions
    assert!(stdout.contains("project_func"));

    // Should show shared function marked as override
    assert!(stdout.contains("shared (overrides global)"));

    // Should show global-only function
    assert!(stdout.contains("global_util"));
}

#[test]
#[serial]
fn test_mcp_inspect_includes_global_tools() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a global runfile
    let home_dir = std::env::var("HOME").unwrap();
    let global_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.test_mcp");
    fs::write(
        &global_runfile,
        r#"
# @desc Global MCP tool
global_tool() {
    echo "Global tool"
}
"#,
    )
    .unwrap();

    // Create a project runfile
    create_runfile(
        temp_dir.path(),
        r#"
# @desc Project MCP tool
project_tool() {
    echo "Project tool"
}
"#,
    );

    // Temporarily swap global runfile (use unique backup name to avoid test interference)
    let original_runfile = std::path::PathBuf::from(&home_dir).join(".runfile");
    let backup_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.backup_test_mcp");
    if original_runfile.exists() {
        fs::rename(&original_runfile, &backup_runfile).ok();
    }
    fs::rename(&global_runfile, &original_runfile).unwrap();

    let output = Command::new(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Restore original runfile
    if original_runfile.exists() {
        fs::remove_file(&original_runfile).ok();
    }
    if backup_runfile.exists() {
        fs::rename(&backup_runfile, &original_runfile).ok();
    }

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    let tools = json["tools"].as_array().expect("tools should be array");

    // Should have both tools
    assert_eq!(tools.len(), 2, "Should have both global and project tools");

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    if !tool_names.contains(&"global_tool") || !tool_names.contains(&"project_tool") {
        eprintln!("Expected global_tool and project_tool, got: {tool_names:?}");
        eprintln!("Full output: {stdout}");
    }

    assert!(tool_names.contains(&"global_tool"));
    assert!(tool_names.contains(&"project_tool"));
}

#[test]
#[serial]
fn test_mcp_project_overrides_global() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    // Create a global runfile with a shared tool
    let home_dir = std::env::var("HOME").unwrap();
    let global_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.test_override");
    fs::write(
        &global_runfile,
        r#"
# @desc Shared tool from project (overrides global)
shared_tool() {
    echo "Global version"
}
"#,
    )
    .unwrap();

    // Create a project runfile that overrides it
    create_runfile(
        temp_dir.path(),
        r#"
# @desc Shared tool from project (overrides global)
shared_tool() {
    echo "Project version"
}
"#,
    );

    // Temporarily swap global runfile (use unique backup name to avoid test interference)
    let original_runfile = std::path::PathBuf::from(&home_dir).join(".runfile");
    let backup_runfile = std::path::PathBuf::from(&home_dir).join(".runfile.backup_test_override");
    if original_runfile.exists() {
        fs::rename(&original_runfile, &backup_runfile).ok();
    }
    fs::rename(&global_runfile, &original_runfile).unwrap();

    let output = Command::new(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Restore original runfile
    if original_runfile.exists() {
        fs::remove_file(&original_runfile).ok();
    }
    if backup_runfile.exists() {
        fs::rename(&backup_runfile, &original_runfile).ok();
    }

    assert!(output.status.success(), "Command failed: {output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("Invalid JSON: {stdout}"));
    let tools = json["tools"].as_array().expect("tools should be array");

    // Debug: print all tools
    if tools.len() != 1 {
        eprintln!("Expected 1 tool, got {}: {:?}", tools.len(), tools);
    }

    // Should have exactly one tool (project overrides global)
    assert_eq!(
        tools.len(),
        1,
        "Should have only one tool (override), got: {tools:?}"
    );

    let tool = &tools[0];
    assert_eq!(tool["name"].as_str().unwrap(), "shared_tool");
    assert_eq!(
        tool["description"].as_str().unwrap(),
        "Shared tool from project (overrides global)",
        "Should use project description, not global"
    );
}

// Note: Tests that manipulate ~/.runfile may need to run serially to avoid race conditions
// Run with: cargo test --test mcp_server_test -- --test-threads=1

/// Test that global ~/.runfile functions can be invoked via MCP when a project Runfile also exists.
/// This is a regression test for the bug where passing `--runfile <project_path>` to the subprocess
/// caused the global merge to be skipped, making global functions fail silently.
#[test]
#[serial]
fn test_mcp_global_function_invocable_with_project_runfile() {
    use std::io::Write;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let home_dir = std::env::var("HOME").unwrap();
    let global_runfile_tmp =
        std::path::PathBuf::from(&home_dir).join(".runfile.test_mcp_global_invoke");
    fs::write(
        &global_runfile_tmp,
        r#"
# @desc Simple test function
ping() {
    echo "pong"
}
"#,
    )
    .unwrap();

    // Create a project runfile so both global and project sources exist
    create_runfile(
        temp_dir.path(),
        r#"
# @desc Project function
check() {
    echo "ok"
}
"#,
    );

    // Swap in our test global runfile
    let original_runfile = std::path::PathBuf::from(&home_dir).join(".runfile");
    let backup_runfile =
        std::path::PathBuf::from(&home_dir).join(".runfile.backup_mcp_global_invoke");
    if original_runfile.exists() {
        fs::rename(&original_runfile, &backup_runfile).ok();
    }
    fs::rename(&global_runfile_tmp, &original_runfile).unwrap();

    // Send an MCP tools/call request for the global function "ping"
    let mut child = Command::new(&binary)
        .arg("--serve-mcp")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let stdin = child.stdin.as_mut().unwrap();

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

    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "ping",
            "arguments": {}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Allow extra time because the MCP server spawns a subprocess to run the function.
    // Existing tests use 500 ms for requests that don't fork; we need more here.
    std::thread::sleep(std::time::Duration::from_secs(2));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();

    // Restore original runfile
    if original_runfile.exists() {
        fs::remove_file(&original_runfile).ok();
    }
    if backup_runfile.exists() {
        fs::rename(&backup_runfile, &original_runfile).ok();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The tools/call response for "ping" should contain "pong" and isError: false
    assert!(
        stdout.contains("pong"),
        "Expected 'pong' in MCP response for global function, got: {stdout}"
    );
    assert!(
        !stdout.contains("\"isError\":true") && !stdout.contains("\"isError\": true"),
        "Global function call should not be an error, got: {stdout}"
    );
}

#[test]
fn test_mcp_hot_reload_emits_tools_list_changed_for_runfile_and_sourced_files() {
    use std::io::Write;
    use std::time::Duration;

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let sourced_path = temp_dir.path().join("extra.run");
    fs::write(
        &sourced_path,
        "# @desc Initial sourced tool\nsourced_one() echo one\n",
    )
    .unwrap();
    create_runfile(
        temp_dir.path(),
        "source ./extra.run\n# @desc First tool\nfirst() echo v1\n",
    );

    let mut child = test_command(&binary)
        .arg("--serve-mcp")
        .env("RUN_MCP_WATCH_INTERVAL_MS", "50")
        .current_dir(temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let stdin = child.stdin.as_mut().unwrap();

    // 1. Initialize and let the watcher take its initial snapshot
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
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(120));

    // 2. Body-only edit should NOT emit list_changed
    create_runfile(
        temp_dir.path(),
        "source ./extra.run\n# @desc First tool\nfirst() echo v2\n",
    );
    std::thread::sleep(Duration::from_millis(150));

    // 3. Edit the sourced file to add a new tool -> should emit list_changed (once)
    fs::write(
        &sourced_path,
        "# @desc Initial sourced tool\nsourced_one() echo one\n# @desc Second sourced tool\nsourced_two(term: str, limit = 15) echo \"$term $limit\"\n",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(180));

    // 4. Re-query tools/list in the same session
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{}", serde_json::to_string(&list_request).unwrap()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(Duration::from_millis(150));

    child.kill().expect("Failed to kill process");
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let messages: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON-RPC line"))
        .collect();

    // Check initialize capability
    let init_resp = messages.iter().find(|m| m["id"] == 1).expect("init resp");
    assert_eq!(
        init_resp["result"]["capabilities"]["tools"]["listChanged"],
        true
    );

    // Exactly one notifications/tools/list_changed notification (for the schema change, not the body edit)
    let notifications: Vec<_> = messages
        .iter()
        .filter(|m| m["method"] == "notifications/tools/list_changed")
        .collect();
    assert_eq!(
        notifications.len(),
        1,
        "Expected 1 list_changed notification, got {notifications:?} in {stdout}"
    );

    // tools/list response includes the hot-reloaded tool with inferred integer schema
    let list_resp = messages.iter().find(|m| m["id"] == 2).expect("list resp");
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    let added = tools
        .iter()
        .find(|t| t["name"] == "sourced_two")
        .expect("hot-reloaded tool sourced_two should be present");
    assert_eq!(
        added["inputSchema"]["properties"]["limit"]["type"],
        "integer"
    );
}

fn run_mcp_session(
    binary: &PathBuf,
    cwd: &std::path::Path,
    requests: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    use std::io::Write;

    let mut child = test_command(binary)
        .arg("--serve-mcp")
        .env("RUN_MCP_WATCH_INTERVAL_MS", "0")
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");

    {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        for req in requests {
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(req).expect("serialize JSON-RPC request")
            )
            .expect("write request");
        }
        stdin.flush().expect("flush stdin");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for MCP server");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("invalid JSON-RPC response line: {e}: {l}"))
        })
        .collect()
}

#[test]
fn test_mcp_tools_list_emits_additional_properties_false_for_all_tools() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @desc Zero-arg tool
ping() echo pong

# @desc Parameterized tool
greet(name: str) echo $name
",
    );

    let responses = run_mcp_session(
        &binary,
        temp_dir.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let tools = responses[0]["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert!(tools.len() >= 5, "expected runfile + builtin tools");

    for tool in tools {
        assert_eq!(
            tool["inputSchema"]["additionalProperties"],
            serde_json::json!(false),
            "tool {} must emit additionalProperties: false",
            tool["name"]
        );
    }
}

#[test]
fn test_mcp_tools_call_rejects_missing_and_null_required_args_without_execution() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();
    let marker_param = temp_dir.path().join("param_ran.marker");
    let marker_legacy = temp_dir.path().join("legacy_ran.marker");

    create_runfile(
        temp_dir.path(),
        &format!(
            r#"
# @desc Deploy service
deploy(env: str, version = "latest") {{
    touch "{}"
    echo "$env:$version"
}}

# @desc Legacy scale
# @arg 1:service string Service name
scale() {{
    touch "{}"
    echo "$1"
}}
"#,
            marker_param.display(),
            marker_legacy.display()
        ),
    );

    let responses = run_mcp_session(
        &binary,
        temp_dir.path(),
        &[
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": "deploy", "arguments": {} }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": "deploy", "arguments": { "env": null } }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "scale", "arguments": {} }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": { "name": "scale", "arguments": { "service": null } }
            }),
        ],
    );

    assert!(!marker_param.exists(), "deploy must not execute");
    assert!(!marker_legacy.exists(), "scale must not execute");
    assert_eq!(responses.len(), 4);

    for (id, expected_name) in [(1, "env"), (2, "env"), (3, "service"), (4, "service")] {
        let resp = responses
            .iter()
            .find(|r| r["id"] == id)
            .expect("response by id");
        assert_eq!(resp["error"]["code"], -32602, "resp: {resp}");
        let msg = resp["error"]["message"].as_str().expect("error message");
        assert!(msg.contains(expected_name), "resp: {resp}");
    }
}

#[test]
fn test_mcp_tools_call_rejects_unknown_and_non_object_args_without_execution() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();
    let marker = temp_dir.path().join("executed.marker");

    create_runfile(
        temp_dir.path(),
        &format!(
            r#"
# @desc Zero arg
ping() {{
    touch "{}"
    echo pong
}}

# @desc Params
greet(name: str) {{
    touch "{}"
    echo "$name"
}}

# @desc Legacy
# @arg 1:target string Target
# @arg 2:branch? string Optional branch
clone() {{
    touch "{}"
    echo "$1"
}}
"#,
            marker.display(),
            marker.display(),
            marker.display()
        ),
    );

    let responses = run_mcp_session(
        &binary,
        temp_dir.path(),
        &[
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": "greet", "arguments": { "name": "ok", "typo": "bad" } }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": "clone", "arguments": { "target": "repo", "branch?": "main" } }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "ping", "arguments": { "unexpected": true } }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": { "name": "ping", "arguments": "not-an-object" }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": { "name": "ping", "arguments": [1, 2] }
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": { "name": "ping", "arguments": null }
            }),
        ],
    );

    assert!(!marker.exists(), "invalid calls must not execute");
    assert_eq!(responses.len(), 6);

    for (id, expected_substr) in [
        (1, "typo"),
        (2, "branch?"),
        (3, "unexpected"),
        (4, "Arguments must be an object"),
        (5, "Arguments must be an object"),
        (6, "Arguments must be an object"),
    ] {
        let resp = responses
            .iter()
            .find(|r| r["id"] == id)
            .expect("response by id");
        assert_eq!(resp["error"]["code"], -32602, "resp: {resp}");
        let msg = resp["error"]["message"].as_str().expect("error message");
        assert!(msg.contains(expected_substr), "resp: {resp}");
    }
}

fn mcp_call(id: i64, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

#[test]
fn test_mcp_tools_call_allows_valid_optional_rest_legacy_and_timeout_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Zero-arg ping
ping() echo "pong"

# @desc Deploy with default (simple)
deploy_simple(env: str, version = "latest") echo "simple:$env:$version"

# @desc Deploy with default (block)
deploy_block(env: str, version = "latest") {
    echo "block:$env:$version"
}

# @desc Rest args
run_many(prefix: str, ...items) {
    echo "rest:$prefix:$items"
}

# @desc Legacy optional arg
# @arg 1:url string Repo URL
# @arg 2:branch? string Optional branch
clone() {
    echo "legacy:$1:${2:-none}"
}
"#,
    );

    let responses = run_mcp_session(
        &binary,
        temp_dir.path(),
        &[
            mcp_call(1, "ping", &serde_json::json!({ "timeout": 10 })),
            mcp_call(2, "ping", &serde_json::json!({ "timeout": null })),
            mcp_call(
                3,
                "deploy_simple",
                &serde_json::json!({ "env": "prod", "timeout": 10 }),
            ),
            mcp_call(
                4,
                "deploy_block",
                &serde_json::json!({ "env": "prod", "version": null }),
            ),
            mcp_call(
                5,
                "run_many",
                &serde_json::json!({ "prefix": "p", "items": ["a", "b"] }),
            ),
            mcp_call(6, "run_many", &serde_json::json!({ "prefix": "only" })),
            mcp_call(
                7,
                "clone",
                &serde_json::json!({ "url": "https://example.com" }),
            ),
            mcp_call(
                8,
                "clone",
                &serde_json::json!({ "url": "https://example.com", "branch": "dev" }),
            ),
        ],
    );

    assert_eq!(responses.len(), 8);

    let expected = [
        (1, "pong"),
        (2, "pong"),
        (3, "simple:prod:latest"),
        (4, "block:prod:latest"),
        (5, "rest:p:a b"),
        (6, "rest:only:"),
        (7, "legacy:https://example.com:none"),
        (8, "legacy:https://example.com:dev"),
    ];

    for (id, needle) in expected {
        let resp = responses
            .iter()
            .find(|r| r["id"] == id)
            .expect("response by id");
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        assert_eq!(resp["result"]["isError"], false, "resp: {resp}");
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        assert!(text.contains(needle), "expected {needle} in {text}");
    }
}
