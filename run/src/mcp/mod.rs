//! Model Context Protocol (MCP) Support for AI Agent Integration
//!
//! This module implements:
//! - JSON schema generation from function metadata (@desc, @arg)
//! - MCP server for JSON-RPC 2.0 communication
//! - Argument mapping from named JSON parameters to positional arguments

mod handlers;
mod mapping;
pub mod tools;

use crate::mcp::handlers::{JsonRpcError, handle_initialize, handle_tools_call, handle_tools_list};
use serde::{Deserialize, Serialize};
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
    request.id.as_ref()?;

    let result = match request.method.as_str() {
        "initialize" => Ok(handle_initialize(request.params)),
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
///
/// Runs an MCP (Model Context Protocol) server that listens for JSON-RPC 2.0
/// requests on stdin and writes responses to stdout. This function runs indefinitely
/// until the input stream is closed.
///
/// # Error Handling
///
/// This function handles errors internally and does not return them:
/// - Parse errors are logged to stderr and returned as JSON-RPC error responses
/// - I/O errors are logged to stderr and the server continues processing
/// - Invalid requests receive JSON-RPC error responses per the MCP specification
pub fn serve_mcp() {
    // Initialize MCP output dir based on Runfile location (or temp as fallback)
    let effective_dir = crate::config::ensure_mcp_output_dir();
    crate::config::set_mcp_output_dir(Some(effective_dir));

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
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
                eprintln!("Error parsing JSON-RPC request: {e}");
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                        data: None,
                    }),
                };

                if let Ok(json) = serde_json::to_string(&error_response) {
                    let _ = writeln!(stdout, "{json}");
                    let _ = stdout.flush();
                }
                continue;
            }
        };

        // Process request
        let response = process_request(request);

        // Only send response if one was returned (not a notification)
        #[allow(clippy::collapsible_if)]
        if let Some(response) = response {
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = writeln!(stdout, "{json}");
                let _ = stdout.flush();
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ast::{ArgMetadata, ArgType, Attribute};
    use tools::extract_function_metadata;

    #[test]
    fn test_extract_function_metadata_with_desc() {
        let attributes = vec![Attribute::Desc("Test function".to_string())];
        let tool = extract_function_metadata("test", &attributes, &[]).unwrap();

        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, "Test function");
        assert!(tool.input_schema.properties.is_empty());
        assert!(tool.input_schema.required.is_empty());
    }

    #[test]
    fn test_extract_function_metadata_without_desc() {
        let attributes = vec![];
        let tool = extract_function_metadata("test", &attributes, &[]);

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

        let tool = extract_function_metadata("scale", &attributes, &[]).unwrap();

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
    fn test_extract_function_metadata_with_params() {
        use crate::ast::Parameter;

        let attributes = vec![Attribute::Desc("Deploy application".to_string())];

        let params = vec![
            Parameter {
                name: "env".to_string(),
                param_type: ArgType::String,
                default_value: None,
                is_rest: false,
            },
            Parameter {
                name: "version".to_string(),
                param_type: ArgType::String,
                default_value: Some("latest".to_string()),
                is_rest: false,
            },
        ];

        let tool = extract_function_metadata("deploy", &attributes, &params).unwrap();

        assert_eq!(tool.name, "deploy");
        assert_eq!(tool.description, "Deploy application");
        assert_eq!(tool.input_schema.properties.len(), 2);
        assert_eq!(tool.input_schema.required.len(), 1); // Only env is required

        let env_param = tool.input_schema.properties.get("env").unwrap();
        assert_eq!(env_param.param_type, "string");

        let version_param = tool.input_schema.properties.get("version").unwrap();
        assert_eq!(version_param.param_type, "string");

        // version should not be required since it has a default
        assert!(tool.input_schema.required.contains(&"env".to_string()));
        assert!(!tool.input_schema.required.contains(&"version".to_string()));
    }

    #[test]
    fn test_extract_function_metadata_with_rest_param() {
        use crate::ast::Parameter;

        let attributes = vec![Attribute::Desc("Echo all arguments".to_string())];

        let params = vec![Parameter {
            name: "args".to_string(),
            param_type: ArgType::String,
            default_value: None,
            is_rest: true,
        }];

        let tool = extract_function_metadata("echo_all", &attributes, &params).unwrap();

        assert_eq!(tool.name, "echo_all");
        assert_eq!(tool.input_schema.properties.len(), 1);
        assert_eq!(tool.input_schema.required.len(), 0); // Rest params are not required

        let args_param = tool.input_schema.properties.get("args").unwrap();
        assert_eq!(args_param.param_type, "array");
    }

    #[test]
    fn test_extract_function_metadata_params_with_arg_descriptions() {
        use crate::ast::Parameter;

        // Hybrid mode: params define types/defaults, @arg provides descriptions
        let attributes = vec![
            Attribute::Desc("Deploy application".to_string()),
            Attribute::Arg(ArgMetadata {
                position: 1,
                name: "env".to_string(),
                arg_type: ArgType::String,
                description: "Target environment (staging|prod)".to_string(),
            }),
            Attribute::Arg(ArgMetadata {
                position: 2,
                name: "version".to_string(),
                arg_type: ArgType::String,
                description: "Version to deploy".to_string(),
            }),
        ];

        let params = vec![
            Parameter {
                name: "env".to_string(),
                param_type: ArgType::String,
                default_value: None,
                is_rest: false,
            },
            Parameter {
                name: "version".to_string(),
                param_type: ArgType::String,
                default_value: Some("latest".to_string()),
                is_rest: false,
            },
        ];

        let tool = extract_function_metadata("deploy", &attributes, &params).unwrap();

        // Params take precedence for types/defaults, @arg provides descriptions
        let env_param = tool.input_schema.properties.get("env").unwrap();
        assert_eq!(env_param.param_type, "string");
        assert_eq!(env_param.description, "Target environment (staging|prod)");

        let version_param = tool.input_schema.properties.get("version").unwrap();
        assert_eq!(version_param.description, "Version to deploy");

        // Only env should be required (version has default)
        assert_eq!(tool.input_schema.required.len(), 1);
        assert!(tool.input_schema.required.contains(&"env".to_string()));
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
