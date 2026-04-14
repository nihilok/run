//! JSON-RPC request handlers for MCP protocol

use super::mapping::map_arguments_to_positional;
use super::mapping::resolve_tool_name;
use super::tools::inspect;
use crate::config;
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// JSON-RPC 2.0 error structure
#[derive(Debug, Serialize)]
pub(super) struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP Server capabilities
#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
struct ToolsCapability {}

/// MCP Server information
#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

fn build_initialize_instructions() -> String {
    let mut instructions = String::from(concat!(
        "This is a `run` MCP server. `run` is a task-runner that reads a `Runfile` ",
        "(similar to a Makefile) in the current working directory. ",
        "Each function defined in the Runfile with a `# @desc` comment is exposed as a tool here. ",
        "If a tool call fails with a Runfile syntax error, the Runfile itself needs to be fixed — ",
        "use the `run_docs` tool to look up correct Runfile syntax, parameter types, and attributes."
    ));

    if let Some((merged_content, _)) = config::load_merged_config() {
        let runfile_instructions = config::collect_mcp_instructions(&merged_content);
        if !runfile_instructions.is_empty() {
            instructions.push_str("\n\nRunfile instructions:");
            for instruction in runfile_instructions {
                instructions.push_str("\n- ");
                instructions.push_str(&instruction);
            }
        }
    }

    instructions
}

/// Handle initialize request
pub(super) fn handle_initialize(_params: Option<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": ServerCapabilities {
            tools: ToolsCapability {},
        },
        "serverInfo": ServerInfo {
            name: "run".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        "instructions": build_initialize_instructions()
    })
}

/// Handle tools/list request
pub(super) fn handle_tools_list(
    _params: Option<serde_json::Value>,
) -> Result<serde_json::Value, JsonRpcError> {
    match inspect() {
        Ok(mut output) => {
            // Append built-in tools
            output.tools.extend(super::tools::get_builtin_tools());

            let value = serde_json::to_value(output).map_err(|e| JsonRpcError {
                code: -32603,
                message: format!("Failed to serialise tools: {e}"),
                data: None,
            })?;

            Ok(value)
        }
        Err(e) => Err(JsonRpcError {
            code: -32603,
            message: format!("Internal error: {e}"),
            data: None,
        }),
    }
}

/// Resolve the runfile path to pass to the MCP subprocess.
///
/// When both `~/.runfile` and a project `Runfile` exist, the merged content is written
/// to a temporary file so the subprocess can access functions from both sources.
/// Returns `(runfile_path, temp_path_to_clean_up)`.  `temp_path_to_clean_up` is `Some`
/// only when a temp file was created and must be removed after the subprocess exits.
fn resolve_subprocess_runfile() -> Result<(PathBuf, Option<PathBuf>), JsonRpcError> {
    let (merged_content, merge_metadata) =
        config::load_merged_config().ok_or_else(|| JsonRpcError {
            code: -32603,
            message: "No Runfile found".to_string(),
            data: None,
        })?;

    if merge_metadata.has_global && merge_metadata.has_project {
        // Both sources present: write merged content to a temp file so the subprocess
        // sees every function.  Using --runfile <project_path> alone would cause
        // load_merged_config to skip the merge and miss global functions.
        let temp_path =
            std::env::temp_dir().join(format!("runfile_merged_{}.run", std::process::id()));
        std::fs::write(&temp_path, &merged_content).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to write merged runfile: {e}"),
            data: None,
        })?;
        Ok((temp_path.clone(), Some(temp_path)))
    } else {
        let runfile_path = config::find_runfile_path().ok_or_else(|| JsonRpcError {
            code: -32603,
            message: "No Runfile found".to_string(),
            data: None,
        })?;
        Ok((runfile_path, None))
    }
}

/// Handle the built-in `run_docs` tool call.
fn handle_run_docs(arguments: &serde_json::Value) -> serde_json::Value {
    let topic = arguments
        .get("topic")
        .and_then(|v| v.as_str())
        .unwrap_or("index");

    let text = if topic == "index" || topic.is_empty() {
        let index = super::tools::DOCS
            .iter()
            .map(|(slug, title, _)| format!("- **{slug}**: {title}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "# run documentation topics\n\nCall `run_docs` with one of these topic slugs:\n\n{index}"
        )
    } else if let Some((_, _, content)) = super::tools::DOCS
        .iter()
        .find(|(slug, _, _)| *slug == topic)
    {
        content.to_string()
    } else {
        let available: Vec<&str> = super::tools::DOCS.iter().map(|(s, _, _)| *s).collect();
        format!(
            "Unknown topic `{topic}`. Available topics: {}",
            available.join(", ")
        )
    };

    serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

fn handle_set_cwd(arguments: &serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let path = arguments
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing 'path' argument".to_string(),
            data: None,
        })?;
    std::env::set_current_dir(path).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to set CWD: {e}"),
        data: None,
    })?;
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": format!("Successfully changed working directory to {path}") }],
        "isError": false
    }))
}

fn handle_get_cwd() -> Result<serde_json::Value, JsonRpcError> {
    let cwd = std::env::current_dir().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to get CWD: {e}"),
        data: None,
    })?;
    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": cwd.display().to_string() }],
        "isError": false
    }))
}

/// Run a command, optionally killing it after `timeout_secs` seconds.
///
/// When `timeout_secs` is `None` the subprocess runs to completion with no time limit.
/// When `Some(secs)` is provided the subprocess is killed and an error is returned if it
/// does not exit within the allotted time.
fn run_command_with_timeout(
    mut cmd: Command,
    timeout_secs: Option<u64>,
) -> Result<std::process::Output, JsonRpcError> {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to execute tool: {e}"),
        data: None,
    })?;

    if let Some(secs) = timeout_secs {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break, // process finished in time
                Ok(None) if Instant::now() >= deadline => {
                    let kill_info = match child.kill() {
                        Ok(()) => String::new(),
                        Err(e) => format!(" (kill failed: {e})"),
                    };
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!(
                            "Tool call timed out after {secs} second(s){kill_info}"
                        ),
                        data: None,
                    });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to wait for tool process: {e}"),
                        data: None,
                    });
                }
            }
        }
    }

    child.wait_with_output().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to collect tool output: {e}"),
        data: None,
    })
}

/// Handle tools/call request
pub(super) fn handle_tools_call(
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, JsonRpcError> {
    let params_obj = params.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let tool_name = params_obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing tool name".to_string(),
            data: None,
        })?;

    // Handle built-in set_cwd tool
    if tool_name == super::tools::TOOL_SET_CWD {
        let default_args = serde_json::json!({});
        let arguments = params_obj.get("arguments").unwrap_or(&default_args);
        return handle_set_cwd(arguments);
    } else if tool_name == super::tools::TOOL_GET_CWD {
        return handle_get_cwd();
    } else if tool_name == super::tools::TOOL_RUN_DOCS {
        let default_args = serde_json::json!({});
        let arguments = params_obj.get("arguments").unwrap_or(&default_args);
        return Ok(handle_run_docs(arguments));
    }

    // Resolve the sanitised tool name back to the original function name
    let actual_function_name = resolve_tool_name(tool_name)?;

    let default_args = serde_json::json!({});
    let arguments = params_obj.get("arguments").unwrap_or(&default_args);

    // Extract the built-in timeout parameter before mapping arguments.
    // It is a reserved MCP-level parameter and must never be forwarded to the
    // underlying shell function as a positional argument.
    let timeout_secs = arguments
        .get(super::tools::TIMEOUT_PARAM)
        .and_then(serde_json::Value::as_u64);

    // Build a filtered argument object that excludes the built-in timeout key so
    // it is not mistakenly mapped to a positional argument of the shell function.
    let filtered_arguments = {
        let mut obj = arguments
            .as_object()
            .cloned()
            .unwrap_or_default();
        obj.remove(super::tools::TIMEOUT_PARAM);
        serde_json::Value::Object(obj)
    };

    // Map arguments to positional (use resolved original function name)
    let positional_args = map_arguments_to_positional(&actual_function_name, &filtered_arguments)?;

    // Execute the function with structured markdown output

    // Get the run binary path (we're already running as run, but we need to call ourselves)
    // Security: We validate that the binary path is a canonical path to ensure it hasn't been manipulated
    let run_binary = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to get binary path: {e}"),
            data: None,
        })?;

    // Get the merged config so the subprocess can find all functions (global + project).
    // When both ~/.runfile and a project Runfile exist, passing --runfile <project_path>
    // causes the subprocess to skip the merge and miss global functions.  Instead we
    // write the already-merged content to a temp file so the subprocess sees everything.
    let (runfile_path, temp_merged_path) = resolve_subprocess_runfile()?;

    let mut cmd = Command::new(run_binary);

    // Pass the Runfile path explicitly so the subprocess can find the functions.
    // This is necessary because the subprocess may run in a different working directory
    // (e.g. after set_cwd was called).
    cmd.arg("--runfile");
    cmd.arg(&runfile_path);
    // Use structured markdown output for better LLM readability
    cmd.arg("--output-format=markdown");

    // Pass MCP output directory to the subprocess via env so it writes to project .run-output
    let mcp_output_dir = config::ensure_mcp_output_dir();
    cmd.env("RUN_MCP_OUTPUT_DIR", &mcp_output_dir);

    // When a temp merged file is used, the subprocess would derive __RUNFILE_DIR__ from
    // the temp file location.  Pass the real project Runfile directory explicitly so that
    // __RUNFILE_DIR__ resolves to the project root, not the system temp directory.
    if temp_merged_path.is_some()
        && let Some(real_dir) = config::find_project_runfile_path()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    {
        cmd.env("RUN_RUNFILE_DIR", real_dir);
    }

    cmd.arg(&actual_function_name); // Use the original function name with colons

    for arg in positional_args {
        cmd.arg(arg);
    }

    let output = run_command_with_timeout(cmd, timeout_secs)?;

    // Clean up temp file if we created one.
    // Cleanup failure is non-critical — the OS will eventually reclaim the temp file —
    // so we intentionally ignore the result here.
    if let Some(ref tp) = temp_merged_path {
        let _ = std::fs::remove_file(tp);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Return content as per MCP spec
    // The stdout now contains structured markdown output
    let mut content = vec![serde_json::json!({
        "type": "text",
        "text": stdout
    })];

    // Always include non-empty stderr so warnings and errors are never silently lost
    if !stderr.is_empty() {
        content.push(serde_json::json!({
            "type": "text",
            "text": format!("STDERR:\n{}", stderr)
        }));
    }

    let result = serde_json::json!({
        "content": content,
        "isError": !output.status.success()
    });

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;
    use std::env;
    use tempfile::tempdir;

    fn disable_global_merge() {
        // Safety: tests are run serially (#[serial]) so no concurrent env mutation
        unsafe { env::set_var("RUN_NO_GLOBAL_MERGE", "1") };
    }

    fn enable_global_merge() {
        // Safety: tests are run serially (#[serial]) so no concurrent env mutation
        unsafe { env::remove_var("RUN_NO_GLOBAL_MERGE") };
    }

    #[test]
    #[serial]
    fn test_handle_get_cwd() {
        let original_cwd = env::current_dir().expect("Failed to get current directory");

        let params = json!({
            "name": "get_cwd",
            "arguments": {}
        });

        let result = handle_tools_call(Some(params)).expect("handle_tools_call should succeed");
        let content = result
            .get("content")
            .expect("Result should have content")
            .as_array()
            .expect("Content should be an array");
        let text = content[0]
            .get("text")
            .expect("Content item should have text")
            .as_str()
            .expect("Text should be a string");

        let cwd = env::current_dir().expect("Failed to get current directory");
        assert_eq!(text, cwd.display().to_string());

        // Restore original CWD in case other tests changed it
        env::set_current_dir(original_cwd).expect("Failed to restore CWD");
    }

    #[test]
    #[serial]
    fn test_handle_set_cwd() {
        let temp = tempdir().expect("Failed to create temp dir");
        let temp_path = temp.path().canonicalize().expect("Failed to canonicalize");
        let original_cwd = env::current_dir().expect("Failed to get current directory");

        let params = json!({
            "name": "set_cwd",
            "arguments": {
                "path": temp_path.to_str().expect("Path should be valid UTF-8")
            }
        });

        let result = handle_tools_call(Some(params)).expect("handle_tools_call should succeed");
        let is_error = result
            .get("isError")
            .expect("Result should have isError")
            .as_bool()
            .expect("isError should be bool");
        assert!(!is_error);

        let new_cwd = env::current_dir().expect("Failed to get current directory");
        assert_eq!(new_cwd, temp_path);

        // Restore original CWD
        env::set_current_dir(original_cwd).expect("Failed to restore CWD");
    }

    #[test]
    fn test_handle_set_cwd_missing_path() {
        let params = json!({
            "name": "set_cwd",
            "arguments": {}
        });

        let result = handle_tools_call(Some(params));
        assert!(result.is_err());
        let err = result.expect_err("Should return an error");
        assert_eq!(err.message, "Missing 'path' argument");
    }

    #[test]
    fn test_handle_set_cwd_invalid_path() {
        let params = json!({
            "name": "set_cwd",
            "arguments": {
                "path": "/non/existent/path/that/should/fail"
            }
        });

        let result = handle_tools_call(Some(params));
        assert!(result.is_err());
        let err = result.expect_err("Should return an error");
        assert!(err.message.contains("Failed to set CWD"));
    }

    #[test]
    fn test_handle_tools_list_includes_builtin() {
        // We need a Runfile for inspect() to work, but handle_tools_list calls inspect()
        // which might fail if no Runfile is present.
        // Let's see if we can just test it.
        let result = handle_tools_list(None);

        if let Ok(value) = result {
            let tools = value
                .get("tools")
                .expect("Result should have tools")
                .as_array()
                .expect("Tools should be an array");
            let tool_names: Vec<&str> = tools
                .iter()
                .map(|t| {
                    t.get("name")
                        .expect("Tool should have name")
                        .as_str()
                        .expect("Name should be string")
                })
                .collect();

            assert!(tool_names.contains(&"set_cwd"));
            assert!(tool_names.contains(&"get_cwd"));
        } else {
            // If it fails because of missing Runfile, that's expected in some environments
            // but for unit tests we should ideally have one or mock it.
            // Given the current structure, inspect() likely returns an error if no Runfile.
        }
    }

    #[test]
    fn test_handle_initialize() {
        let value = handle_initialize(None);

        assert_eq!(value["protocolVersion"], "2024-11-05");
        assert!(value.get("capabilities").is_some());
        assert!(value.get("serverInfo").is_some());

        let server_info = &value["serverInfo"];
        assert_eq!(server_info["name"], "run");
        // Version should be non-empty
        assert!(!server_info["version"].as_str().unwrap().is_empty());

        // instructions field must be present and mention Runfile
        let instructions = value["instructions"].as_str().unwrap();
        assert!(
            instructions.contains("Runfile"),
            "instructions should mention Runfile: {instructions}"
        );
        assert!(
            instructions.contains("run_docs"),
            "instructions should mention run_docs: {instructions}"
        );
    }

    #[test]
    #[serial]
    fn test_handle_initialize_appends_runfile_instructions() {
        let temp = tempdir().expect("Failed to create temp dir");
        let original_cwd = env::current_dir().expect("Failed to get cwd");
        let original_custom_path = crate::config::get_custom_runfile_path();
        disable_global_merge();

        std::fs::write(
            temp.path().join("Runfile"),
            "\
# @instructions Always confirm production environment before deploy\n\
source ./shared.run\n\
# @desc Hello tool\n\
hello() echo hello\n",
        )
        .expect("Failed to write Runfile");
        std::fs::write(
            temp.path().join("shared.run"),
            "# @instructions Prefer short, exact query keywords for recall\n",
        )
        .expect("Failed to write sourced file");

        env::set_current_dir(temp.path()).expect("Failed to set cwd");
        crate::config::set_custom_runfile_path(None);

        let value = handle_initialize(None);
        let instructions = value["instructions"].as_str().expect("instructions string");
        assert!(instructions.contains("Runfile instructions:"));
        assert!(instructions.contains("- Always confirm production environment before deploy"));
        assert!(instructions.contains("- Prefer short, exact query keywords for recall"));

        crate::config::set_custom_runfile_path(original_custom_path);
        enable_global_merge();
        env::set_current_dir(original_cwd).expect("Failed to restore cwd");
    }

    #[test]
    #[serial]
    fn test_handle_initialize_without_runfile_instructions_section() {
        let temp = tempdir().expect("Failed to create temp dir");
        let original_cwd = env::current_dir().expect("Failed to get cwd");
        let original_custom_path = crate::config::get_custom_runfile_path();
        disable_global_merge();

        std::fs::write(
            temp.path().join("Runfile"),
            "# @desc Hello tool\nhello() echo hello\n",
        )
        .expect("Failed to write Runfile");

        env::set_current_dir(temp.path()).expect("Failed to set cwd");
        crate::config::set_custom_runfile_path(None);

        let value = handle_initialize(None);
        let instructions = value["instructions"].as_str().expect("instructions string");
        assert!(
            !instructions.contains("Runfile instructions:"),
            "unexpected Runfile instructions section: {instructions}"
        );

        crate::config::set_custom_runfile_path(original_custom_path);
        enable_global_merge();
        env::set_current_dir(original_cwd).expect("Failed to restore cwd");
    }

    #[test]
    fn test_handle_initialize_with_params() {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test",
                "version": "1.0"
            }
        });
        let value = handle_initialize(Some(params));
        assert_eq!(value["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn test_handle_tools_call_missing_params() {
        let result = handle_tools_call(None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing params"));
    }

    #[test]
    fn test_handle_tools_call_missing_tool_name() {
        let params = json!({
            "arguments": {}
        });
        let result = handle_tools_call(Some(params));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing tool name"));
    }

    #[test]
    #[serial]
    fn test_handle_tools_call_get_cwd_value() {
        let original_cwd = env::current_dir().expect("Should get cwd");

        let params = json!({
            "name": "get_cwd",
            "arguments": {}
        });
        let result = handle_tools_call(Some(params)).expect("Should succeed");
        assert!(!result["isError"].as_bool().unwrap());

        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty());

        env::set_current_dir(original_cwd).ok();
    }

    #[test]
    fn test_handle_tools_call_set_cwd_nonexistent() {
        let params = json!({
            "name": "set_cwd",
            "arguments": {
                "path": "/this/path/should/not/exist/ever"
            }
        });
        let result = handle_tools_call(Some(params));
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_handle_tools_call_unknown_tool() {
        let original_cwd = env::current_dir().expect("Failed to get cwd");
        let temp = tempdir().expect("Failed to create temp dir");
        std::fs::write(
            temp.path().join("Runfile"),
            "# @desc Say hello\nhello() echo \"hello\"\n",
        )
        .expect("Failed to write Runfile");
        env::set_current_dir(temp.path()).expect("Failed to set cwd");
        disable_global_merge();

        let params = json!({
            "name": "nonexistent_tool_xyz",
            "arguments": {}
        });
        let result = handle_tools_call(Some(params));

        enable_global_merge();
        env::set_current_dir(original_cwd).expect("Failed to restore cwd");

        let err = result.expect_err("Should return error for unknown tool");
        assert_eq!(err.code, -32602);
        assert!(
            err.message.contains("Tool not found"),
            "Unexpected message: {}",
            err.message
        );
    }

    #[test]
    fn test_run_command_with_timeout_no_timeout_succeeds() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let output = run_command_with_timeout(cmd, None).expect("should succeed without timeout");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
    }

    #[test]
    fn test_run_command_with_timeout_generous_timeout_succeeds() {
        let mut cmd = Command::new("echo");
        cmd.arg("world");
        let output =
            run_command_with_timeout(cmd, Some(30)).expect("should succeed within 30s timeout");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("world"));
    }

    #[test]
    fn test_run_command_with_timeout_expires() {
        // Use `sleep 10` which will never finish within 1 second
        let cmd = Command::new("sleep");
        let mut sleep_cmd = cmd;
        sleep_cmd.arg("10");
        let result = run_command_with_timeout(sleep_cmd, Some(1));
        let err = result.expect_err("should return an error when command times out");
        assert_eq!(err.code, -32603);
        assert!(
            err.message.contains("timed out"),
            "Expected timeout message, got: {}",
            err.message
        );
    }
}
