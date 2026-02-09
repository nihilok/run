//! Integration tests for RFC004 MCP tool schema generation

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;

#[test]
fn test_mcp_inspect_with_params() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Deploy application to environment
deploy(env, version = "latest") {
    echo "Deploying $version to $env"
}
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
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");

    // Verify structure
    let tools = json["tools"].as_array().expect("tools should be array");
    assert_eq!(tools.len(), 1, "Should have one tool");

    let deploy_tool = &tools[0];
    assert_eq!(deploy_tool["name"], "deploy");
    assert_eq!(
        deploy_tool["description"],
        "Deploy application to environment"
    );

    let properties = &deploy_tool["inputSchema"]["properties"];
    assert!(properties["env"].is_object());
    assert!(properties["version"].is_object());

    // env should be required, version should not (has default)
    let required = deploy_tool["inputSchema"]["required"]
        .as_array()
        .expect("required should be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "env");
}

#[test]
fn test_mcp_inspect_with_rest_params() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @desc Run command in container
docker_exec(container, ...command) {
    docker exec $container $command
}
",
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    let tools = json["tools"].as_array().expect("tools should be array");
    let tool = &tools[0];

    let properties = &tool["inputSchema"]["properties"];
    assert_eq!(properties["container"]["type"], "string");
    assert_eq!(properties["command"]["type"], "array");

    // Only container should be required
    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("required should be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "container");
}

#[test]
fn test_mcp_inspect_hybrid_mode_with_descriptions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @desc Scale a service
# @arg service The service to scale
# @arg replicas Number of instances
scale(service, replicas: int = 1) {
    docker compose scale $service=$replicas
}
",
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    let tools = json["tools"].as_array().expect("tools should be array");
    let tool = &tools[0];

    // Check that types come from params
    let properties = &tool["inputSchema"]["properties"];
    assert_eq!(properties["service"]["type"], "string");
    assert_eq!(properties["replicas"]["type"], "integer");

    // Check that descriptions come from @arg
    assert_eq!(properties["service"]["description"], "The service to scale");
    assert_eq!(properties["replicas"]["description"], "Number of instances");

    // Only service should be required (replicas has default)
    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("required should be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "service");
}

#[test]
fn test_mcp_inspect_backward_compatibility() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @desc Restart services
# @arg 1:service string The service name
restart() {
    docker compose restart $1
}
",
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    let tools = json["tools"].as_array().expect("tools should be array");
    let tool = &tools[0];

    // Old @arg style should still work
    let properties = &tool["inputSchema"]["properties"];
    assert_eq!(properties["service"]["type"], "string");
    assert_eq!(properties["service"]["description"], "The service name");

    let required = tool["inputSchema"]["required"]
        .as_array()
        .expect("required should be array");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "service");
}

#[test]
fn test_mcp_inspect_multiple_functions() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @desc Deploy application
deploy(env, version = "latest") {
    echo "Deploying $version to $env"
}

# @desc Scale service
scale(service, replicas: int = 1) {
    docker compose scale $service=$replicas
}

# Function without description - should not appear in MCP
build() {
    docker compose build
}
"#,
    );

    let output = test_command(&binary)
        .arg("--inspect")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");
    let tools = json["tools"].as_array().expect("tools should be array");

    // Should have 2 tools (deploy and scale, not build)
    assert_eq!(tools.len(), 2);

    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(names.contains(&"deploy"));
    assert!(names.contains(&"scale"));
}
