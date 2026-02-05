//! JSON-RPC request handlers for MCP protocol

use super::mapping::resolve_tool_name;
use super::mapping::map_arguments_to_positional;
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
        Ok(output) => serde_json::to_value(output).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to serialise tools: {}", e),
            data: None,
        }),
        Err(e) => Err(JsonRpcError {
            code: -32603,
            message: format!("Internal error: {}", e),
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

    // Resolve the sanitised tool name back to the original function name
    let actual_function_name = resolve_tool_name(tool_name)?;

    let default_args = serde_json::json!({});
    let arguments = params_obj
        .get("arguments")
        .unwrap_or(&default_args);

    // Map arguments to positional (use resolved original function name)
    let positional_args = map_arguments_to_positional(&actual_function_name, arguments)?;

    // Execute the function with structured markdown output
    use std::process::Command;
    use crate::config;

    // Get the run binary path (we're already running as run, but we need to call ourselves)
    // Security: We validate that the binary path is a canonical path to ensure it hasn't been manipulated
    let run_binary = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to get binary path: {}", e),
            data: None,
        })?;

    // Get the Runfile path so the subprocess can find the functions
    // This is necessary because the subprocess may run in a different working directory
    let runfile_path = config::find_runfile_path()
        .ok_or_else(|| JsonRpcError {
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
    cmd.arg(&actual_function_name);  // Use the original function name with colons

    for arg in positional_args {
        cmd.arg(arg);
    }

    let output = cmd.output().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to execute tool: {}", e),
        data: None,
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Return content as per MCP spec
    // The stdout now contains structured markdown output
    let mut content = vec![
        serde_json::json!({
            "type": "text",
            "text": stdout
        })
    ];

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
