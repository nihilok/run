//! Model Context Protocol (MCP) Support for AI Agent Integration
//! 
//! This module implements:
//! - JSON schema generation from function metadata (@desc, @arg)
//! - MCP server for JSON-RPC 2.0 communication
//! - Argument mapping from named JSON parameters to positional arguments

use crate::ast::{ArgType, Attribute, Statement};
use crate::{config, parser};
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
    pub input_schema: InputSchema,
}

/// Root structure for inspect output
#[derive(Debug, Serialize, Deserialize)]
pub struct InspectOutput {
    pub tools: Vec<Tool>,
}

/// Extract metadata from function attributes
fn extract_function_metadata(
    name: &str,
    attributes: &[Attribute],
) -> Tool {
    let mut description = String::new();
    let mut properties = HashMap::new();
    let mut required = Vec::new();
    
    // Process attributes
    for attr in attributes {
        match attr {
            Attribute::Desc(desc) => {
                description = desc.clone();
            }
            Attribute::Arg(arg_meta) => {
                let param_type = match arg_meta.arg_type {
                    ArgType::String => "string",
                    ArgType::Integer => "integer",
                    ArgType::Boolean => "boolean",
                };
                
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
    
    Tool {
        name: name.to_string(),
        description,
        input_schema: InputSchema {
            schema_type: "object".to_string(),
            properties,
            required,
        },
    }
}

/// Check if function should be included based on OS platform filters
fn should_include_function(attributes: &[Attribute]) -> bool {
    use crate::ast::OsPlatform;
    
    let os_attributes: Vec<&OsPlatform> = attributes
        .iter()
        .filter_map(|attr| {
            if let Attribute::Os(platform) = attr {
                Some(platform)
            } else {
                None
            }
        })
        .collect();
    
    // If no OS attributes, include the function
    if os_attributes.is_empty() {
        return true;
    }
    
    // Check if any OS attribute matches the current platform
    for platform in os_attributes {
        match platform {
            OsPlatform::Windows => {
                if cfg!(windows) {
                    return true;
                }
            }
            OsPlatform::Linux => {
                if cfg!(target_os = "linux") {
                    return true;
                }
            }
            OsPlatform::MacOS => {
                if cfg!(target_os = "macos") {
                    return true;
                }
            }
            OsPlatform::Unix => {
                if cfg!(unix) {
                    return true;
                }
            }
        }
    }
    
    false
}

/// Generate inspection output from Runfile
pub fn inspect() -> Result<InspectOutput, String> {
    let config_content = config::load_config_or_exit();
    
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
                if should_include_function(&attributes) {
                    tools.push(extract_function_metadata(&name, &attributes));
                }
            }
            Statement::BlockFunctionDef {
                name,
                attributes,
                ..
            } => {
                if should_include_function(&attributes) {
                    tools.push(extract_function_metadata(&name, &attributes));
                }
            }
            _ => {}
        }
    }
    
    Ok(InspectOutput { tools })
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

/// Serve MCP protocol over stdio (placeholder for Phase 3)
pub fn serve_mcp() {
    eprintln!("MCP server not yet implemented");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgMetadata, ArgType, Attribute};

    #[test]
    fn test_extract_function_metadata_with_desc() {
        let attributes = vec![Attribute::Desc("Test function".to_string())];
        let tool = extract_function_metadata("test", &attributes);
        
        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, "Test function");
        assert!(tool.input_schema.properties.is_empty());
        assert!(tool.input_schema.required.is_empty());
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
        
        let tool = extract_function_metadata("scale", &attributes);
        
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
    fn test_should_include_function_no_os_filter() {
        let attributes = vec![Attribute::Desc("Test".to_string())];
        assert!(should_include_function(&attributes));
    }

    #[test]
    fn test_should_include_function_with_unix_filter() {
        use crate::ast::OsPlatform;
        let attributes = vec![Attribute::Os(OsPlatform::Unix)];
        
        // Should include on Unix systems
        if cfg!(unix) {
            assert!(should_include_function(&attributes));
        } else {
            assert!(!should_include_function(&attributes));
        }
    }
}
