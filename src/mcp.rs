//! Model Context Protocol (MCP) Support for AI Agent Integration
//! 
//! This module implements:
//! - JSON schema generation from function metadata (@desc, @arg)
//! - MCP server for JSON-RPC 2.0 communication
//! - Argument mapping from named JSON parameters to positional arguments

use crate::ast::{Attribute, Statement};
use crate::{config, parser, utils};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON Schema for a tool parameter
#[derive(Debug, Serialize, Deserialize)]
pub struct ParameterSchema {
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
}

/// JSON Schema for tool input
#[derive(Debug, Serialize, Deserialize)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, ParameterSchema>,
    pub required: Vec<String>,
}

/// A tool definition for MCP
#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: InputSchema,
}

/// Root structure for inspect output
#[derive(Debug, Serialize, Deserialize)]
pub struct InspectOutput {
    pub tools: Vec<Tool>,
}

/// Extract metadata from function attributes
/// Returns None if the function has no @desc attribute
fn extract_function_metadata(
    name: &str,
    attributes: &[Attribute],
) -> Option<Tool> {
    let mut description: Option<String> = None;
    let mut properties = HashMap::new();
    let mut required = Vec::new();
    
    // Process attributes
    for attr in attributes {
        match attr {
            Attribute::Desc(desc) => {
                description = Some(desc.clone());
            }
            Attribute::Arg(arg_meta) => {
                let param_type = utils::arg_type_to_json_type(&arg_meta.arg_type);
                
                properties.insert(
                    arg_meta.name.clone(),
                    ParameterSchema {
                        param_type: param_type.to_string(),
                        description: arg_meta.description.clone(),
                    },
                );
                
                required.push(arg_meta.name.clone());
            }
            _ => {}
        }
    }
    
    // Only return a tool if it has a description
    description.map(|desc| {
        // Sanitize tool name: MCP spec requires [a-zA-Z0-9_-] only
        // Replace colons with double underscores
        let sanitized_name = name.replace(':', "__");

        Tool {
            name: sanitized_name,
            description: desc,
            input_schema: InputSchema {
                schema_type: "object".to_string(),
                properties,
                required,
            },
        }
    })
}

/// Generate inspection output from Runfile
pub fn inspect() -> Result<InspectOutput, String> {
    let config_content = match config::load_config() {
        Some(content) => content,
        None => return Ok(InspectOutput { tools: Vec::new() }), // No Runfile = no tools
    };

    let program = parser::parse_script(&config_content)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let mut tools = Vec::new();
    
    for statement in program.statements {
        match statement {
            Statement::SimpleFunctionDef {
                name,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(&attributes) {
                    if let Some(tool) = extract_function_metadata(&name, &attributes) {
                        tools.push(tool);
                    }
                }
            }
            Statement::BlockFunctionDef {
                name,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(&attributes) {
                    if let Some(tool) = extract_function_metadata(&name, &attributes) {
                        tools.push(tool);
                    }
                }
            }
            _ => {}
        }
    }
    
    Ok(InspectOutput { tools })
}

/// Resolve a sanitized tool name back to the original function name
/// This is needed because MCP requires [a-zA-Z0-9_-] but we support colons in function names
fn resolve_tool_name(sanitized_name: &str) -> Result<String, JsonRpcError> {
    let config_content = config::load_config()
        .ok_or_else(|| JsonRpcError {
            code: -32603,
            message: "No Runfile found".to_string(),
            data: None,
        })?;

    let program = parser::parse_script(&config_content)
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Parse error: {}", e),
            data: None,
        })?;

    // Look for a function whose sanitized name matches
    for statement in program.statements {
        let (name, attributes) = match statement {
            Statement::SimpleFunctionDef { name, attributes, .. } => (name, attributes),
            Statement::BlockFunctionDef { name, attributes, .. } => (name, attributes),
            _ => continue,
        };

        // Check if this function has @desc (would be exposed as tool)
        if attributes.iter().any(|a| matches!(a, Attribute::Desc(_))) {
            let tool_name = name.replace(':', "__");
            if tool_name == sanitized_name {
                return Ok(name);
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: format!("Tool not found: {}", sanitized_name),
        data: None,
    })
}

/// Print inspection output as JSON
pub fn print_inspect() {
    match inspect() {
        Ok(output) => {
            let json = serde_json::to_string_pretty(&output)
                .expect("Failed to serialize to JSON");
            println!("{}", json);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

// ========== MCP Server Implementation ==========

use std::io::{BufRead, BufReader, Write};

/// JSON-RPC 2.0 request structure
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
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
fn handle_initialize(
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
fn handle_tools_list(
    _params: Option<serde_json::Value>,
) -> Result<serde_json::Value, JsonRpcError> {
    match inspect() {
        Ok(output) => Ok(serde_json::to_value(output).unwrap()),
        Err(e) => Err(JsonRpcError {
            code: -32603,
            message: format!("Internal error: {}", e),
            data: None,
        }),
    }
}

/// Map JSON arguments to positional shell arguments
fn map_arguments_to_positional(
    tool_name: &str,
    json_args: &serde_json::Value,
) -> Result<Vec<String>, JsonRpcError> {
    // Load the Runfile to get function metadata
    let config_content = match crate::config::load_config() {
        Some(content) => content,
        None => {
            return Err(JsonRpcError {
                code: -32603,
                message: "No Runfile found".to_string(),
                data: None,
            });
        }
    };
    
    let program = match crate::parser::parse_script(&config_content) {
        Ok(prog) => prog,
        Err(e) => {
            return Err(JsonRpcError {
                code: -32603,
                message: format!("Parse error: {}", e),
                data: None,
            });
        }
    };
    
    // Find the function and get its @arg attributes
    let mut arg_mapping: HashMap<usize, String> = HashMap::new();
    
    for statement in program.statements {
        let (name, attributes) = match statement {
            Statement::SimpleFunctionDef { name, attributes, .. } => (name, attributes),
            Statement::BlockFunctionDef { name, attributes, .. } => (name, attributes),
            _ => continue,
        };
        
        if name == tool_name {
            // Extract argument metadata
            for attr in attributes {
                if let Attribute::Arg(arg_meta) = attr {
                    arg_mapping.insert(arg_meta.position, arg_meta.name.clone());
                }
            }
            break;
        }
    }
    
    // If no @arg attributes found, return empty arguments
    if arg_mapping.is_empty() {
        return Ok(Vec::new());
    }
    
    // Get the JSON object with arguments
    let args_obj = json_args.as_object().ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Arguments must be an object".to_string(),
        data: None,
    })?;
    
    // Build positional arguments array
    let max_position = *arg_mapping.keys().max().unwrap_or(&0);
    let mut positional_args = vec![String::new(); max_position];
    
    for (position, param_name) in arg_mapping.iter() {
        if let Some(value) = args_obj.get(param_name) {
            let arg_str = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => String::new(),
                _ => value.to_string(),
            };
            
            if *position > 0 && *position <= positional_args.len() {
                positional_args[position - 1] = arg_str;
            }
        }
    }
    
    Ok(positional_args)
}

/// Handle tools/call request
fn handle_tools_call(
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
    
    // Resolve the sanitized tool name back to the original function name
    let actual_function_name = resolve_tool_name(tool_name)?;

    let default_args = serde_json::json!({});
    let arguments = params_obj
        .get("arguments")
        .unwrap_or(&default_args);
    
    // Map arguments to positional (use sanitized name for lookup)
    let positional_args = map_arguments_to_positional(tool_name, arguments)?;
    
    // Execute the function
    use std::process::Command;
    
    // Get the run binary path (we're already running as run, but we need to call ourselves)
    // Security: We validate that the binary path is a canonical path to ensure it hasn't been manipulated
    let run_binary = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to get binary path: {}", e),
            data: None,
        })?;
    
    let mut cmd = Command::new(run_binary);
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
    // Include stderr in content if execution failed for better debugging
    let mut content = vec![
        serde_json::json!({
            "type": "text",
            "text": stdout
        })
    ];
    
    if !output.status.success() && !stderr.is_empty() {
        content.push(serde_json::json!({
            "type": "text",
            "text": format!("Error: {}", stderr)
        }));
    }
    
    let result = serde_json::json!({
        "content": content,
        "isError": !output.status.success()
    });
    
    Ok(result)
}

/// Process a single JSON-RPC request
fn process_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: format!("Invalid JSON-RPC version: {}", request.jsonrpc),
                data: None,
            }),
        });
    }

    // Handle notifications (requests without an id)
    if request.id.is_none() {
        // Process notification but don't send a response
        match request.method.as_str() {
            "initialized" | "notifications/initialized" => {
                // Acknowledged, no response needed
            }
            _ => {
                // Unknown notification, just ignore
            }
        }
        return None;
    }

    let result = match request.method.as_str() {
        "initialize" => handle_initialize(request.params),
        "tools/list" => handle_tools_list(request.params),
        "tools/call" => handle_tools_call(request.params),
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
            data: None,
        }),
    };
    
    Some(match result {
        Ok(res) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(res),
            error: None,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(err),
        },
    })
}

/// Serve MCP protocol over stdio
pub fn serve_mcp() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);
    
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin: {}", e);
                continue;
            }
        };
        
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }
        
        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Error parsing JSON-RPC request: {}", e);
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                
                if let Ok(json) = serde_json::to_string(&error_response) {
                    let _ = writeln!(stdout, "{}", json);
                    let _ = stdout.flush();
                }
                continue;
            }
        };
        
        // Process request
        let response = process_request(request);
        
        // Only send response if one was returned (not a notification)
        if let Some(response) = response {
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = writeln!(stdout, "{}", json);
                let _ = stdout.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgMetadata, ArgType, Attribute};

    #[test]
    fn test_extract_function_metadata_with_desc() {
        let attributes = vec![Attribute::Desc("Test function".to_string())];
        let tool = extract_function_metadata("test", &attributes).unwrap();

        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, "Test function");
        assert!(tool.input_schema.properties.is_empty());
        assert!(tool.input_schema.required.is_empty());
    }

    #[test]
    fn test_extract_function_metadata_without_desc() {
        let attributes = vec![];
        let tool = extract_function_metadata("test", &attributes);

        assert!(tool.is_none());
    }

    #[test]
    fn test_extract_function_metadata_with_args() {
        let attributes = vec![
            Attribute::Desc("Scale service".to_string()),
            Attribute::Arg(ArgMetadata {
                position: 1,
                name: "service".to_string(),
                arg_type: ArgType::String,
                description: "Service name".to_string(),
            }),
            Attribute::Arg(ArgMetadata {
                position: 2,
                name: "replicas".to_string(),
                arg_type: ArgType::Integer,
                description: "Number of replicas".to_string(),
            }),
        ];
        
        let tool = extract_function_metadata("scale", &attributes).unwrap();

        assert_eq!(tool.name, "scale");
        assert_eq!(tool.description, "Scale service");
        assert_eq!(tool.input_schema.properties.len(), 2);
        assert_eq!(tool.input_schema.required.len(), 2);
        
        let service_param = tool.input_schema.properties.get("service").unwrap();
        assert_eq!(service_param.param_type, "string");
        assert_eq!(service_param.description, "Service name");
        
        let replicas_param = tool.input_schema.properties.get("replicas").unwrap();
        assert_eq!(replicas_param.param_type, "integer");
        assert_eq!(replicas_param.description, "Number of replicas");
    }

    #[test]
    fn test_process_request_invalid_jsonrpc_version() {
        let request = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: None,
        };

        let response = process_request(request);
        assert!(response.is_some());

        let response = response.unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32600);
        assert!(error.message.contains("Invalid JSON-RPC version"));
    }

    #[test]
    fn test_process_request_valid_jsonrpc_version() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            })),
        };

        let response = process_request(request);
        assert!(response.is_some());

        let response = response.unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }
}
