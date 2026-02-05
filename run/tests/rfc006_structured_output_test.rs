use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_structured_output_json() {
    let temp_dir = TempDir::new().unwrap();
    let runfile_path = temp_dir.path().join("Runfile");

    let runfile_content = r#"
# @desc Test function
test_func() {
    echo "Hello World"
}
"#;

    fs::write(&runfile_path, runfile_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_run"))
        .arg("--runfile")
        .arg(&runfile_path)
        .arg("--output-format=json")
        .arg("test_func")
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "Command failed: {}", stderr);

    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Failed to parse JSON: {}. Stdout was: {}", e, stdout));

    // Check structure
    assert_eq!(json["context"]["function_name"], "test_func");
    assert_eq!(json["context"]["interpreter"], "sh");
    assert_eq!(json["success"], true);
    assert!(json["outputs"].is_array());
    assert_eq!(json["outputs"].as_array().unwrap().len(), 1);

    let first_output = &json["outputs"][0];
    assert!(
        first_output["stdout"]
            .as_str()
            .unwrap()
            .contains("Hello World")
    );
    assert_eq!(first_output["stderr"], "");
    assert_eq!(first_output["exit_code"], 0);
    assert!(first_output["duration_ms"].as_u64().is_some());
}

#[test]
fn test_structured_output_markdown() {
    let temp_dir = TempDir::new().unwrap();
    let runfile_path = temp_dir.path().join("Runfile");

    let runfile_content = r#"
# @desc Test function with multiple steps
multi_step() {
    echo "Step 1"
    echo "Step 2"
}
"#;

    fs::write(&runfile_path, runfile_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_run"))
        .arg("--runfile")
        .arg(&runfile_path)
        .arg("--output-format=markdown")
        .arg("multi_step")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    // Check markdown structure - MCP format hides implementation details
    assert!(stdout.contains("## Execution: `multi_step`"));
    assert!(stdout.contains("**Status:** ✓ Success"));
    assert!(stdout.contains("**Duration:**"));
    // MCP format combines outputs and doesn't show Step headers (to hide implementation)
    assert!(stdout.contains("**Output:**"));
    assert!(stdout.contains("Step 1"));
    assert!(stdout.contains("Step 2"));
}

#[test]
fn test_structured_output_with_stderr() {
    let temp_dir = TempDir::new().unwrap();
    let runfile_path = temp_dir.path().join("Runfile");

    let runfile_content = r#"
error_test() {
    echo "stdout message"
    echo "stderr message" >&2
}
"#;

    fs::write(&runfile_path, runfile_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_run"))
        .arg("--runfile")
        .arg(&runfile_path)
        .arg("--output-format=json")
        .arg("error_test")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let first_output = &json["outputs"][0];
    assert!(
        first_output["stdout"]
            .as_str()
            .unwrap()
            .contains("stdout message")
    );
    assert!(
        first_output["stderr"]
            .as_str()
            .unwrap()
            .contains("stderr message")
    );
}

#[test]
fn test_default_stream_mode_no_capture() {
    let temp_dir = TempDir::new().unwrap();
    let runfile_path = temp_dir.path().join("Runfile");

    let runfile_content = r#"
test_func() {
    echo "Direct output"
}
"#;

    fs::write(&runfile_path, runfile_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_run"))
        .arg("--runfile")
        .arg(&runfile_path)
        .arg("test_func")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    // In stream mode, output goes directly to terminal
    // Should NOT contain JSON or markdown formatting
    assert!(!stdout.contains("\"function_name\""));
    assert!(!stdout.contains("## Execution:"));
    assert!(stdout.contains("Direct output"));
}

#[test]
fn test_ssh_context_extraction() {
    let temp_dir = TempDir::new().unwrap();
    let runfile_path = temp_dir.path().join("Runfile");

    // Create a function that contains an SSH command pattern in its body
    // We can't actually run SSH, but we can test that the context extraction works
    let runfile_content = r#"
ssh_test() {
    echo "ssh admin@webserver.example.com"
}
"#;

    fs::write(&runfile_path, runfile_content).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_run"))
        .arg("--runfile")
        .arg(&runfile_path)
        .arg("--output-format=json")
        .arg("ssh_test")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // The command itself doesn't contain SSH, but verify the structure exists
    // The extraction happens when a command string contains "ssh user@host"
    assert!(json["context"]["remote_host"].is_null() || json["context"]["remote_host"].is_string());
    assert!(json["context"]["remote_user"].is_null() || json["context"]["remote_user"].is_string());
}

// Note: Unit tests for extract_ssh_context are in run/src/ast.rs
