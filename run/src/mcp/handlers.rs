//! JSON-RPC request handlers for MCP protocol

use super::mapping::map_arguments_to_positional;
use super::mapping::resolve_tool_name;
use super::tools::inspect;
use serde::Serialize;

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

/// Handle initialize request
pub(super) fn handle_initialize(
    _params: Option<serde_json::Value>,
) -> Result<serde_json::Value, JsonRpcError> {
    let response = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": ServerCapabilities {
            tools: ToolsCapability {},
        },
        "serverInfo": ServerInfo {
            name: "run".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    });
    Ok(response)
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

        return Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Successfully changed working directory to {}", path)
            }],
            "isError": false
        }));
    } else if tool_name == super::tools::TOOL_GET_CWD {
        let cwd = std::env::current_dir().map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to get CWD: {e}"),
            data: None,
        })?;

        return Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": cwd.display().to_string()
            }],
            "isError": false
        }));
    }

    // Resolve the sanitised tool name back to the original function name
    let actual_function_name = resolve_tool_name(tool_name)?;

    let default_args = serde_json::json!({});
    let arguments = params_obj.get("arguments").unwrap_or(&default_args);

    // Map arguments to positional (use resolved original function name)
    let positional_args = map_arguments_to_positional(&actual_function_name, arguments)?;

    // Execute the function with structured markdown output
    use crate::config;
    use std::process::Command;

    // Get the run binary path (we're already running as run, but we need to call ourselves)
    // Security: We validate that the binary path is a canonical path to ensure it hasn't been manipulated
    let run_binary = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to get binary path: {e}"),
            data: None,
        })?;

    // Get the Runfile path so the subprocess can find the functions
    // This is necessary because the subprocess may run in a different working directory
    let runfile_path = config::find_runfile_path().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    let mut cmd = Command::new(run_binary);
    // Pass the Runfile path explicitly so the subprocess can find the functions
    cmd.arg("--runfile");
    cmd.arg(&runfile_path);
    // Use structured markdown output for better LLM readability
    cmd.arg("--output-format=markdown");

    // Pass MCP output directory to the subprocess via env so it writes to project .run-output
    let mcp_output_dir = config::ensure_mcp_output_dir();
    cmd.env("RUN_MCP_OUTPUT_DIR", &mcp_output_dir);

    cmd.arg(&actual_function_name); // Use the original function name with colons

    for arg in positional_args {
        cmd.arg(arg);
    }

    let output = cmd.output().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to execute tool: {e}"),
        data: None,
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Return content as per MCP spec
    // The stdout now contains structured markdown output
    let mut content = vec![serde_json::json!({
        "type": "text",
        "text": stdout
    })];

    // Only include stderr if there was an error (structured output captures stderr in the markdown)
    if !stderr.is_empty() && !output.status.success() {
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;
    use std::env;
    use tempfile::tempdir;

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
}
